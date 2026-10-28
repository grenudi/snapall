// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application use case, written against the port only.

use thiserror::Error;

use crate::course::{Course, Unattended};
use crate::domain::{
    AreaName, AreaOption, Change, Description, FileVersion, Point, RestoreSummary, SnapshotId,
    SnapshotInfo,
};
use crate::plan::{plan, Plan, PlanError};
use crate::port::{BackendError, SnapshotBackend};

/// What happened for one config.
#[derive(Debug)]
pub struct Outcome {
    pub area: AreaName,
    pub result: Result<SnapshotId, BackendError>,
}

/// One outcome per planned config, in plan order.
#[derive(Debug)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
}

impl Report {
    pub fn failures(&self) -> usize {
        self.outcomes.iter().filter(|o| o.result.is_err()).count()
    }

    pub fn is_success(&self) -> bool {
        self.failures() == 0
    }
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error(transparent)]
    Plan(#[from] PlanError),
}

/// Ask the backend what exists, then decide what to do. No snapshot is taken.
pub fn plan_snapshots(
    backend: &dyn SnapshotBackend,
    only: Option<&[AreaName]>,
) -> Result<Plan, SnapshotError> {
    let available = backend.list_areas()?;
    Ok(plan(only, &available)?)
}

/// Execute a plan. One failing config does not stop the others: every config gets its
/// attempt and the caller sees every outcome.
pub fn run_plan(
    backend: &dyn SnapshotBackend,
    plan: &Plan,
    description: &Description,
    point: Option<&Point>,
) -> Report {
    run_plan_along(backend, plan, description, point, &Unattended).0
}

/// [`run_plan`], one step per config, following a [`Course`]: told before each, and stoppable between
/// them. The second answer is whether it was stopped before the end; the report then has the outcomes of
/// the steps that were taken.
pub fn run_plan_along(
    backend: &dyn SnapshotBackend,
    plan: &Plan,
    description: &Description,
    point: Option<&Point>,
    course: &dyn Course,
) -> (Report, bool) {
    let total = plan.targets().len();
    let mut outcomes = Vec::with_capacity(total);
    for (done, area) in plan.targets().iter().enumerate() {
        if !course.proceed(done, total, area.as_str()) {
            return (Report { outcomes }, true);
        }
        outcomes.push(Outcome {
            area: area.clone(),
            // every snapshot of one action carries the same restore point, so they can be found
            // together again and put back with one action
            result: match point {
                Some(point) => backend.create_snapshot_in(area, description, point),
                None => backend.create_snapshot(area, description),
            },
        });
    }
    (Report { outcomes }, false)
}

pub fn snapshot_all(
    backend: &dyn SnapshotBackend,
    only: Option<&[AreaName]>,
    description: &Description,
    point: Option<&Point>,
) -> Result<Report, SnapshotError> {
    let plan = plan_snapshots(backend, only)?;
    Ok(run_plan(backend, &plan, description, point))
}

/// Fail fast, with our own message, when `config` is not one the backend manages.
fn ensure_area(backend: &dyn SnapshotBackend, area: &AreaName) -> Result<(), SnapshotError> {
    if backend.list_areas()?.contains(area) {
        Ok(())
    } else {
        Err(PlanError::UnknownAreas(vec![area.clone()]).into())
    }
}

/// The snapshots of one config, oldest first.
pub fn snapshots_of(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
) -> Result<Vec<SnapshotInfo>, SnapshotError> {
    ensure_area(backend, area)?;
    Ok(backend.list_snapshots(area)?)
}

/// The snapshots of every config, configs sorted by name. A failure for any config is an
/// error: a partial listing would look complete.
pub fn all_snapshots(
    backend: &dyn SnapshotBackend,
) -> Result<Vec<(AreaName, Vec<SnapshotInfo>)>, SnapshotError> {
    let mut areas = backend.list_areas()?;
    areas.sort();
    areas.dedup();
    areas
        .into_iter()
        .map(|area| {
            let snapshots = backend.list_snapshots(&area)?;
            Ok((area, snapshots))
        })
        .collect()
}

/// Delete one snapshot of an existing config.
pub fn delete_snapshot(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
) -> Result<(), SnapshotError> {
    ensure_area(backend, area)?;
    Ok(backend.delete_snapshot(area, id)?)
}

/// Why a restore was refused or failed.
#[derive(Debug, Error)]
pub enum RestoreError {
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("{area} has no snapshot #{id}")]
    UnknownSnapshot { area: AreaName, id: SnapshotId },
    /// Putting the whole system root back while it is running would leave files, kernel and
    /// boot menu out of step with each other. Single paths are fine; the system itself is
    /// restored from the boot menu or a live system.
    #[error("`{area}` is the system root: restoring all of it while it runs is not safe (restore single paths, or restore the system from the boot menu)")]
    SystemRoot { area: AreaName },
    #[error("`{path}` is not an absolute path without `..`")]
    BadPath { path: String },
    #[error("`{path}` is outside {area}, which is {location}")]
    OutsideArea {
        area: AreaName,
        path: String,
        location: String,
    },
    #[error("the safety snapshot could not be taken, so nothing was restored: {0}")]
    SafetySnapshot(BackendError),
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// What restoring would do, before anything is touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Where the config lives.
    pub location: String,
    /// Restoring all of it is refused (see [`RestoreError::SystemRoot`]); single paths still work.
    pub is_system_root: bool,
    /// The paths that differ now, limited to the requested path if there was one.
    pub changes: Vec<Change>,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The snapshot of the state just before, which undoes this restore.
    pub safety_snapshot: SnapshotId,
    pub summary: RestoreSummary,
}

pub(crate) fn is_root(location: &str) -> bool {
    location.trim_end_matches('/').is_empty()
}

/// A path that cannot escape: absolute, no `..` component, no control line breaks.
fn clean(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains(['\0', '\n', '\r'])
        && !path.split('/').any(|part| part == "..")
}

/// Whether `path` is `base` or lies under it.
fn under(base: &str, path: &str) -> bool {
    let base = base.trim_end_matches('/');
    base.is_empty()
        || path == base
        || path
            .strip_prefix(base)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn checked(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<String, RestoreError> {
    ensure_area(backend, area)?;
    if !backend.list_snapshots(area)?.iter().any(|s| s.id == id) {
        return Err(RestoreError::UnknownSnapshot {
            area: area.clone(),
            id,
        });
    }
    let location = backend.area_location(area)?;
    if let Some(path) = path {
        if !clean(path) {
            return Err(RestoreError::BadPath {
                path: path.to_owned(),
            });
        }
        if !under(&location, path) {
            return Err(RestoreError::OutsideArea {
                area: area.clone(),
                path: path.to_owned(),
                location,
            });
        }
    }
    Ok(location)
}

/// What restoring snapshot `id` of `config` would change. Read-only.
pub fn preview_restore(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<Preview, RestoreError> {
    let location = checked(backend, area, id, path)?;
    let mut changes = backend.changes_since(area, id)?;
    if let Some(path) = path {
        changes.retain(|change| under(path, &change.path));
    }
    Ok(Preview {
        is_system_root: is_root(&location),
        location,
        changes,
    })
}

/// Put `config` (or only `path` in it) back to how snapshot `id` had it.
///
/// Two rules make this safe to offer. The state just before is snapshotted first, and if that
/// fails nothing is restored, so a restore can always be undone. And the system root is never
/// restored whole: only single paths of it.
pub fn restore_snapshot(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<Restored, RestoreError> {
    restore_snapshot_with(backend, area, id, path, None)
}

/// How the snapshot taken just before a restore is named and tagged, when it is one of several taken
/// by one action (restoring a whole restore point), so that undoing the action is one action too.
#[derive(Debug, Clone)]
pub struct Safety {
    pub point: Point,
    pub note: Description,
}

/// [`restore_snapshot`], with the safety snapshot tagged and named as given.
pub fn restore_snapshot_with(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
    safety: Option<&Safety>,
) -> Result<Restored, RestoreError> {
    let location = checked(backend, area, id, path)?;
    if path.is_none() && is_root(&location) {
        return Err(RestoreError::SystemRoot { area: area.clone() });
    }
    let safety_snapshot = match safety {
        Some(safety) => backend.create_snapshot_in(area, &safety.note, &safety.point),
        None => {
            let note = Description::parse(&format!("before restoring #{id}")).map_err(|_| {
                RestoreError::BadPath {
                    path: format!("#{id}"),
                }
            })?;
            backend.create_snapshot(area, &note)
        }
    }
    .map_err(RestoreError::SafetySnapshot)?;
    let summary = backend.undo_changes(area, id, path)?;
    Ok(Restored {
        safety_snapshot,
        summary,
    })
}

/// Why a file's history could not be read.
#[derive(Debug, Error)]
pub enum HistoryError {
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("`{path}` is not an absolute path without `..`")]
    BadPath { path: String },
    #[error("no protected area contains `{path}`")]
    NoArea { path: String },
    /// The path is under a config's directory but on another subvolume, which that config's
    /// snapshots do not include. Saying "did not exist" here would be wrong: it never could have.
    #[error("`{path}` is in a different part of the disk than {area} (`{location}`), so no snapshot of {area} contains it")]
    OtherLocation {
        path: String,
        area: AreaName,
        location: String,
    },
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// A file, the config that covers it, and how it stood in each snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistory {
    pub area: AreaName,
    pub path: String,
    /// Oldest first, as snapper lists them.
    pub versions: Vec<FileVersion>,
}

/// The config whose subvolume holds `path`: the one with the longest matching mount point, so a
/// file in `/home` belongs to `home` and not to `root`, which also contains it.
pub fn area_covering(backend: &dyn SnapshotBackend, path: &str) -> Result<AreaName, HistoryError> {
    if !clean(path) {
        return Err(HistoryError::BadPath {
            path: path.to_owned(),
        });
    }
    // The most specific config whose subvolume really holds the file (by length of mount point),
    // and, failing that, the most specific one that merely has it under its directory.
    let mut covering: Option<(usize, AreaName)> = None;
    let mut only_by_name: Option<(usize, AreaName, String)> = None;
    for area in backend.list_areas().map_err(SnapshotError::from)? {
        let location = backend.area_location(&area)?;
        if !under(&location, path) {
            continue;
        }
        let length = location.trim_end_matches('/').len();
        if backend.covers(&area, path)? {
            if covering
                .as_ref()
                .is_none_or(|(longest, _)| length > *longest)
            {
                covering = Some((length, area));
            }
        } else if only_by_name
            .as_ref()
            .is_none_or(|(longest, ..)| length > *longest)
        {
            only_by_name = Some((length, area, location));
        }
    }
    match (covering, only_by_name) {
        (Some((_, area)), _) => Ok(area),
        (None, Some((_, area, location))) => Err(HistoryError::OtherLocation {
            path: path.to_owned(),
            area,
            location,
        }),
        (None, None) => Err(HistoryError::NoArea {
            path: path.to_owned(),
        }),
    }
}

/// How `path` stood in every snapshot of `config`.
pub fn file_versions_of(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    path: &str,
) -> Result<Vec<FileVersion>, HistoryError> {
    if !clean(path) {
        return Err(HistoryError::BadPath {
            path: path.to_owned(),
        });
    }
    ensure_area(backend, area)?;
    let location = backend.area_location(area)?;
    if !under(&location, path) {
        return Err(HistoryError::NoArea {
            path: path.to_owned(),
        });
    }
    if !backend.covers(area, path)? {
        return Err(HistoryError::OtherLocation {
            path: path.to_owned(),
            area: area.clone(),
            location,
        });
    }
    Ok(backend.file_versions(area, path)?)
}

/// The history of one file: which config covers it, and how it stood in each snapshot.
pub fn file_history(
    backend: &dyn SnapshotBackend,
    path: &str,
) -> Result<FileHistory, HistoryError> {
    let area = area_covering(backend, path)?;
    let versions = file_versions_of(backend, &area, path)?;
    Ok(FileHistory {
        area,
        path: path.to_owned(),
        versions,
    })
}

/// Why an area could not be protected.
#[derive(Debug, Error)]
pub enum SetupError {
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("`{path}` is not an absolute path without `..`")]
    BadPath { path: String },
    #[error("{path} is already protected (by `{area}`)")]
    AlreadyProtected { path: String, area: AreaName },
    #[error("no name could be chosen for the area at {path}")]
    NoName { path: String },
    /// It failed, and what had been started was undone: the disk is as it was.
    #[error("{0}")]
    Failed(BackendError),
    /// It failed half way and could not be undone: something was left behind, and says what.
    #[error("setting up {path} failed ({problem}) and the part that was done could not be undone ({undo}): the area is left as `{area}`")]
    LeftBehind {
        path: String,
        area: AreaName,
        // boxed: this is the rare case, and two whole errors would make every Result larger
        problem: Box<BackendError>,
        undo: Box<BackendError>,
    },
}

/// Start protecting the area mounted at `mount_point`: create what snapshots of it are kept in, and
/// turn off snapshots on a schedule, so that they are taken when the person asks and nothing piles up
/// on its own. If the second step fails the first is undone, so a half-made area never lingers.
/// Returns the name it was given.
pub fn protect_area(
    backend: &dyn SnapshotBackend,
    mount_point: &str,
) -> Result<AreaName, SetupError> {
    if !clean(mount_point) {
        return Err(SetupError::BadPath {
            path: mount_point.to_owned(),
        });
    }
    let areas = backend.list_areas().map_err(SnapshotError::from)?;
    for area in &areas {
        let location = backend.area_location(area).map_err(SnapshotError::from)?;
        if location.trim_end_matches('/') == mount_point.trim_end_matches('/') {
            return Err(SetupError::AlreadyProtected {
                path: mount_point.to_owned(),
                area: area.clone(),
            });
        }
    }
    let name =
        crate::coverage::suggest_name(mount_point, &areas).ok_or_else(|| SetupError::NoName {
            path: mount_point.to_owned(),
        })?;
    backend
        .create_area(&name, mount_point)
        .map_err(SetupError::Failed)?;
    if let Err(problem) = backend.set_area_option(&name, AreaOption::TimelineCreate(false)) {
        return match backend.delete_area(&name) {
            Ok(()) => Err(SetupError::Failed(problem)),
            Err(undo) => Err(SetupError::LeftBehind {
                path: mount_point.to_owned(),
                area: name,
                problem: Box::new(problem),
                undo: Box::new(undo),
            }),
        };
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::InMemoryBackend;

    fn desc() -> Description {
        Description::parse("test").unwrap()
    }

    #[test]
    fn snapshots_every_area_by_default() {
        let backend = InMemoryBackend::with_areas(&["root", "home"]);
        let report = snapshot_all(&backend, None, &desc(), None).unwrap();
        assert!(report.is_success());
        assert_eq!(report.outcomes.len(), 2);
        assert_eq!(backend.snapshots().len(), 2);
    }

    #[test]
    fn only_restricts_the_targets() {
        let backend = InMemoryBackend::with_areas(&["root", "home"]);
        let only = [AreaName::parse("home").unwrap()];
        let report = snapshot_all(&backend, Some(&only), &desc(), None).unwrap();
        assert_eq!(report.outcomes.len(), 1);
        assert_eq!(backend.snapshots()[0].0.as_str(), "home");
    }

    #[test]
    fn one_failure_does_not_stop_the_rest_and_is_reported() {
        let backend = InMemoryBackend::with_areas(&["root", "home"]).failing_on("home");
        let report = snapshot_all(&backend, None, &desc(), None).unwrap();
        assert_eq!(report.outcomes.len(), 2);
        assert_eq!(report.failures(), 1);
        assert!(!report.is_success());
        assert_eq!(backend.snapshots().len(), 1);
    }

    #[test]
    fn planning_alone_takes_no_snapshot() {
        let backend = InMemoryBackend::with_areas(&["root"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        assert_eq!(plan.targets().len(), 1);
        assert!(backend.snapshots().is_empty());
    }

    #[test]
    fn a_backend_without_areas_is_a_plan_error() {
        let backend = InMemoryBackend::with_areas(&[]);
        assert!(matches!(
            snapshot_all(&backend, None, &desc(), None),
            Err(SnapshotError::Plan(PlanError::NoAreas))
        ));
    }

    #[test]
    fn listing_and_deleting_go_through_the_use_cases() {
        let backend = InMemoryBackend::with_areas(&["root", "home"]);
        snapshot_all(&backend, None, &desc(), None).unwrap();
        let root = AreaName::parse("root").unwrap();

        let listed = snapshots_of(&backend, &root).unwrap();
        assert_eq!(listed.len(), 1);
        delete_snapshot(&backend, &root, listed[0].id).unwrap();
        assert!(snapshots_of(&backend, &root).unwrap().is_empty());

        let all = all_snapshots(&backend).unwrap();
        assert_eq!(
            all.iter()
                .map(|(c, s)| (c.as_str(), s.len()))
                .collect::<Vec<_>>(),
            [("home", 1), ("root", 0)]
        );
    }

    #[test]
    fn an_unknown_area_is_our_error_not_the_backends() {
        let backend = InMemoryBackend::with_areas(&["root"]);
        let nope = AreaName::parse("nope").unwrap();
        assert!(matches!(
            snapshots_of(&backend, &nope),
            Err(SnapshotError::Plan(PlanError::UnknownAreas(_)))
        ));
        assert!(matches!(
            delete_snapshot(&backend, &nope, SnapshotId::new(1).unwrap()),
            Err(SnapshotError::Plan(PlanError::UnknownAreas(_)))
        ));
    }

    // ---- restoring -----------------------------------------------------------------------

    use crate::domain::ChangeKind;

    fn home() -> AreaName {
        AreaName::parse("home").unwrap()
    }

    fn id(n: u64) -> SnapshotId {
        SnapshotId::new(n).unwrap()
    }

    /// `home` with a.txt = "one", snapshotted as #1, then a.txt edited and b.txt added.
    fn edited_home() -> InMemoryBackend {
        let backend = InMemoryBackend::with_areas(&["home"]);
        backend.write_file("home", "/mnt/home/a.txt", "one");
        backend.write_file("home", "/mnt/home/c.txt", "kept");
        backend.create_snapshot(&home(), &desc()).unwrap();
        backend.write_file("home", "/mnt/home/a.txt", "two");
        backend.write_file("home", "/mnt/home/b.txt", "new");
        backend.write_file("home", "/mnt/home/c.txt", "edited");
        backend
    }

    #[test]
    fn a_preview_lists_what_would_change_and_changes_nothing() {
        let backend = edited_home();
        let preview = preview_restore(&backend, &home(), id(1), None).unwrap();
        let kinds: Vec<_> = preview
            .changes
            .iter()
            .map(|c| (c.path.as_str(), c.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("/mnt/home/a.txt", ChangeKind::Changed),
                ("/mnt/home/b.txt", ChangeKind::Added),
                ("/mnt/home/c.txt", ChangeKind::Changed),
            ]
        );
        assert!(!preview.is_system_root);
        assert_eq!(
            backend.read_file("home", "/mnt/home/a.txt").as_deref(),
            Some("two")
        );
        assert_eq!(backend.snapshots().len(), 1, "a preview takes no snapshot");
    }

    #[test]
    fn restoring_puts_everything_back_after_taking_a_safety_snapshot() {
        let backend = edited_home();
        let restored = restore_snapshot(&backend, &home(), id(1), None).unwrap();
        assert_eq!(
            backend.read_file("home", "/mnt/home/a.txt").as_deref(),
            Some("one")
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt"),
            None,
            "added since: removed"
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/c.txt").as_deref(),
            Some("kept")
        );
        assert_eq!(
            (restored.summary.modified, restored.summary.deleted),
            (2, 1)
        );
        let note = backend
            .snapshots()
            .into_iter()
            .find(|(_, snapshot, _)| *snapshot == restored.safety_snapshot)
            .unwrap();
        assert_eq!(note.2, "before restoring #1");
    }

    #[test]
    fn a_restore_can_itself_be_undone_through_its_safety_snapshot() {
        let backend = edited_home();
        let restored = restore_snapshot(&backend, &home(), id(1), None).unwrap();
        restore_snapshot(&backend, &home(), restored.safety_snapshot, None).unwrap();
        assert_eq!(
            backend.read_file("home", "/mnt/home/a.txt").as_deref(),
            Some("two")
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("new")
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/c.txt").as_deref(),
            Some("edited")
        );
    }

    #[test]
    fn a_restore_can_be_limited_to_one_path() {
        let backend = edited_home();
        let preview = preview_restore(&backend, &home(), id(1), Some("/mnt/home/a.txt")).unwrap();
        assert_eq!(preview.changes.len(), 1);
        restore_snapshot(&backend, &home(), id(1), Some("/mnt/home/a.txt")).unwrap();
        assert_eq!(
            backend.read_file("home", "/mnt/home/a.txt").as_deref(),
            Some("one")
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/c.txt").as_deref(),
            Some("edited"),
            "not asked for"
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("new")
        );
    }

    #[test]
    fn the_system_root_is_never_restored_whole_and_nothing_is_touched_when_refusing() {
        let backend = InMemoryBackend::with_areas(&["root"]).with_root_area("root");
        let root = AreaName::parse("root").unwrap();
        backend.write_file("root", "/etc/fstab", "good");
        backend.create_snapshot(&root, &desc()).unwrap();
        backend.write_file("root", "/etc/fstab", "broken");

        let refused = restore_snapshot(&backend, &root, id(1), None).unwrap_err();
        assert!(
            matches!(refused, RestoreError::SystemRoot { .. }),
            "{refused}"
        );
        assert_eq!(
            backend.read_file("root", "/etc/fstab").as_deref(),
            Some("broken")
        );
        assert_eq!(
            backend.snapshots().len(),
            1,
            "no safety snapshot for a refused restore"
        );

        // A single path of the root is a different matter, and is allowed.
        let preview = preview_restore(&backend, &root, id(1), None).unwrap();
        assert!(
            preview.is_system_root,
            "the preview says so, so the UI can explain"
        );
        restore_snapshot(&backend, &root, id(1), Some("/etc/fstab")).unwrap();
        assert_eq!(
            backend.read_file("root", "/etc/fstab").as_deref(),
            Some("good")
        );
    }

    #[test]
    fn paths_cannot_escape_the_area_and_a_refusal_changes_nothing() {
        let backend = edited_home();
        for (path, expect_outside) in [
            ("/etc/passwd", true),
            ("/mnt/homeless/a.txt", true),
            ("/mnt/home/../etc/passwd", false),
            ("relative/path", false),
            ("/mnt/home/a\nb", false),
        ] {
            let error = restore_snapshot(&backend, &home(), id(1), Some(path)).unwrap_err();
            assert_eq!(
                matches!(error, RestoreError::OutsideArea { .. }),
                expect_outside,
                "{path}: {error}"
            );
            assert!(
                !matches!(error, RestoreError::Backend(_)),
                "{path} must be caught before the backend"
            );
        }
        assert_eq!(backend.snapshots().len(), 1, "no safety snapshot was taken");
        assert_eq!(
            backend.read_file("home", "/mnt/home/a.txt").as_deref(),
            Some("two")
        );
    }

    #[test]
    fn an_unknown_snapshot_or_area_is_refused_before_anything_changes() {
        let backend = edited_home();
        assert!(matches!(
            restore_snapshot(&backend, &home(), id(9), None),
            Err(RestoreError::UnknownSnapshot { .. })
        ));
        let nope = AreaName::parse("nope").unwrap();
        assert!(matches!(
            restore_snapshot(&backend, &nope, id(1), None),
            Err(RestoreError::Snapshot(_))
        ));
        assert_eq!(backend.snapshots().len(), 1);
    }

    #[test]
    fn if_the_safety_snapshot_cannot_be_taken_nothing_is_restored() {
        let backend = edited_home();
        backend.refuse_snapshots_of("home");
        let error = restore_snapshot(&backend, &home(), id(1), None).unwrap_err();
        assert!(matches!(error, RestoreError::SafetySnapshot(_)), "{error}");
        assert_eq!(
            backend.read_file("home", "/mnt/home/a.txt").as_deref(),
            Some("two")
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("new")
        );
    }

    // ---- a file's history --------------------------------------------------------------------

    use crate::domain::FileState;

    fn states(history: &FileHistory) -> Vec<(u64, FileState)> {
        history
            .versions
            .iter()
            .map(|v| (v.snapshot.id.get(), v.state))
            .collect()
    }

    #[test]
    fn the_area_with_the_longest_mount_point_covers_a_file() {
        let backend = InMemoryBackend::with_areas(&["home", "root"]).with_root_area("root");
        assert_eq!(
            area_covering(&backend, "/mnt/home/a.txt").unwrap().as_str(),
            "home"
        );
        assert_eq!(
            area_covering(&backend, "/etc/fstab").unwrap().as_str(),
            "root"
        );
        // a sibling that merely starts with the same letters is not inside
        assert_eq!(
            area_covering(&backend, "/mnt/homeless/a.txt")
                .unwrap()
                .as_str(),
            "root"
        );
    }

    #[test]
    fn a_file_no_area_covers_or_a_path_that_could_escape_is_refused() {
        let backend = InMemoryBackend::with_areas(&["home"]);
        assert!(matches!(
            area_covering(&backend, "/etc/fstab"),
            Err(HistoryError::NoArea { .. })
        ));
        for bad in ["relative.txt", "/mnt/home/../etc/passwd", "/mnt/home/a\nb"] {
            assert!(
                matches!(
                    area_covering(&backend, bad),
                    Err(HistoryError::BadPath { .. })
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_files_history_says_in_each_snapshot_how_it_compares_with_now() {
        let backend = InMemoryBackend::with_areas(&["home"]);
        backend.write_file("home", "/mnt/home/a.txt", "one");
        backend.create_snapshot(&home(), &desc()).unwrap(); // #1: one
        backend.write_file("home", "/mnt/home/a.txt", "two");
        backend.create_snapshot(&home(), &desc()).unwrap(); // #2: two
        backend.write_file("home", "/mnt/home/b.txt", "new"); // not in either snapshot

        let a = file_history(&backend, "/mnt/home/a.txt").unwrap();
        assert_eq!(a.area, home());
        assert_eq!(states(&a), [(1, FileState::Changed), (2, FileState::Same)]);
        let b = file_history(&backend, "/mnt/home/b.txt").unwrap();
        assert_eq!(
            states(&b),
            [(1, FileState::AbsentThen), (2, FileState::AbsentThen)]
        );

        backend.remove_file("home", "/mnt/home/a.txt");
        let gone = file_history(&backend, "/mnt/home/a.txt").unwrap();
        assert_eq!(
            states(&gone),
            [(1, FileState::DeletedSince), (2, FileState::DeletedSince)]
        );
    }

    #[test]
    fn only_a_version_that_would_put_something_back_is_restorable() {
        assert!(FileState::Changed.restorable() && FileState::DeletedSince.restorable());
        assert!(!FileState::Same.restorable() && !FileState::AbsentThen.restorable());
    }

    #[test]
    fn a_history_for_a_path_outside_the_given_area_is_refused() {
        let backend = InMemoryBackend::with_areas(&["home", "root"]).with_root_area("root");
        assert!(matches!(
            file_versions_of(&backend, &home(), "/etc/fstab"),
            Err(HistoryError::NoArea { .. })
        ));
    }

    // ---- what a snapshot cannot contain ---------------------------------------------------------

    #[test]
    fn a_file_on_a_separate_location_is_not_attributed_to_the_area_above_it() {
        // root is `/`, and /mnt/home is a separate subvolume with no config of its own
        let backend = InMemoryBackend::with_areas(&["root"])
            .with_root_area("root")
            .with_separate_location("/mnt/home");
        let error = area_covering(&backend, "/mnt/home/u/notes.txt").unwrap_err();
        assert!(
            matches!(&error, HistoryError::OtherLocation { area, .. } if area.as_str() == "root"),
            "{error}"
        );
        assert!(
            error
                .to_string()
                .contains("no snapshot of root contains it"),
            "{error}"
        );
        // a file of the root subvolume itself is still covered by it
        assert_eq!(
            area_covering(&backend, "/etc/fstab").unwrap().as_str(),
            "root"
        );
        // and asking for the history through the config directly says the same, not "did not exist"
        let direct = file_versions_of(
            &backend,
            &AreaName::parse("root").unwrap(),
            "/mnt/home/u/notes.txt",
        );
        assert!(
            matches!(direct, Err(HistoryError::OtherLocation { .. })),
            "{direct:?}"
        );
    }

    #[test]
    fn once_the_separate_location_has_its_own_area_that_area_covers_its_files() {
        let backend = InMemoryBackend::with_areas(&["home", "root"])
            .with_root_area("root")
            .with_separate_location("/mnt/home");
        assert_eq!(
            area_covering(&backend, "/mnt/home/u/notes.txt")
                .unwrap()
                .as_str(),
            "home"
        );
        assert_eq!(
            area_covering(&backend, "/etc/fstab").unwrap().as_str(),
            "root"
        );
    }

    // ---- protecting an area ---------------------------------------------------------------------

    #[test]
    fn protecting_an_area_sets_it_up_named_after_it_and_stops_snapshots_on_a_schedule() {
        let backend = InMemoryBackend::with_areas(&["root"]).with_root_area("root");
        let name = protect_area(&backend, "/home").unwrap();
        assert_eq!(name.as_str(), "home");
        assert!(backend.list_areas().unwrap().contains(&name));
        assert_eq!(backend.area_location(&name).unwrap(), "/home");
        assert_eq!(
            backend.timeline_create("home"),
            Some(false),
            "snapshots happen when asked, not on a schedule"
        );
        // the new area takes part in "snapshot everything" straight away
        let plan = plan_snapshots(&backend, None).unwrap();
        assert_eq!(plan.targets().len(), 2);
    }

    #[test]
    fn an_area_that_is_already_protected_is_refused_and_nothing_changes() {
        let backend = InMemoryBackend::with_areas(&["root", "home"]).with_root_area("root");
        let before = backend.list_areas().unwrap();
        // the in-memory `home` config lives at /mnt/home
        let error = protect_area(&backend, "/mnt/home/").unwrap_err();
        assert!(
            matches!(&error, SetupError::AlreadyProtected { area, .. } if area.as_str() == "home"),
            "{error}"
        );
        assert_eq!(backend.list_areas().unwrap(), before);
    }

    #[test]
    fn a_name_that_is_taken_is_not_reused() {
        // a config called `home` exists for somewhere else
        let backend = InMemoryBackend::with_areas(&["home"]);
        let name = protect_area(&backend, "/home").unwrap();
        assert_eq!(name.as_str(), "home_2");
    }

    #[test]
    fn if_the_second_step_fails_the_first_is_undone_so_nothing_half_made_lingers() {
        let backend = InMemoryBackend::with_areas(&["root"]).with_root_area("root");
        backend.refuse_option_changes();
        let error = protect_area(&backend, "/home").unwrap_err();
        assert!(matches!(error, SetupError::Failed(_)), "{error}");
        assert_eq!(
            backend.list_areas().unwrap().len(),
            1,
            "the new area was removed again"
        );
    }

    #[test]
    fn if_even_the_undo_fails_what_is_left_behind_is_named() {
        let backend = InMemoryBackend::with_areas(&["root"]).with_root_area("root");
        backend.refuse_option_changes();
        backend.refuse_removals();
        let error = protect_area(&backend, "/home").unwrap_err();
        let text = error.to_string();
        assert!(
            matches!(&error, SetupError::LeftBehind { area, .. } if area.as_str() == "home"),
            "{text}"
        );
        assert!(
            text.contains("could not be undone") && text.contains("`home`"),
            "{text}"
        );
    }

    #[test]
    fn a_path_that_could_escape_is_refused_before_anything_is_created() {
        let backend = InMemoryBackend::with_areas(&["root"]).with_root_area("root");
        for bad in ["home", "/home/../etc", ""] {
            assert!(
                matches!(protect_area(&backend, bad), Err(SetupError::BadPath { .. })),
                "{bad:?}"
            );
        }
        assert_eq!(backend.list_areas().unwrap().len(), 1);
    }
}
