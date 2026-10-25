// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application use case, written against the port only.

use thiserror::Error;

use crate::domain::{
    Change, ConfigName, Description, FileVersion, RestoreSummary, SnapshotId, SnapshotInfo,
};
use crate::plan::{plan, Plan, PlanError};
use crate::port::{BackendError, SnapshotBackend};

/// What happened for one config.
#[derive(Debug)]
pub struct Outcome {
    pub config: ConfigName,
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
    only: Option<&[ConfigName]>,
) -> Result<Plan, SnapshotError> {
    let available = backend.list_configs()?;
    Ok(plan(only, &available)?)
}

/// Execute a plan. One failing config does not stop the others: every config gets its
/// attempt and the caller sees every outcome.
pub fn run_plan(backend: &dyn SnapshotBackend, plan: &Plan, description: &Description) -> Report {
    let outcomes = plan
        .targets()
        .iter()
        .map(|config| Outcome {
            config: config.clone(),
            result: backend.create_snapshot(config, description),
        })
        .collect();
    Report { outcomes }
}

pub fn snapshot_all(
    backend: &dyn SnapshotBackend,
    only: Option<&[ConfigName]>,
    description: &Description,
) -> Result<Report, SnapshotError> {
    let plan = plan_snapshots(backend, only)?;
    Ok(run_plan(backend, &plan, description))
}

/// Fail fast, with our own message, when `config` is not one the backend manages.
fn ensure_config(backend: &dyn SnapshotBackend, config: &ConfigName) -> Result<(), SnapshotError> {
    if backend.list_configs()?.contains(config) {
        Ok(())
    } else {
        Err(PlanError::UnknownConfigs(vec![config.clone()]).into())
    }
}

/// The snapshots of one config, oldest first.
pub fn snapshots_of(
    backend: &dyn SnapshotBackend,
    config: &ConfigName,
) -> Result<Vec<SnapshotInfo>, SnapshotError> {
    ensure_config(backend, config)?;
    Ok(backend.list_snapshots(config)?)
}

/// The snapshots of every config, configs sorted by name. A failure for any config is an
/// error: a partial listing would look complete.
pub fn all_snapshots(
    backend: &dyn SnapshotBackend,
) -> Result<Vec<(ConfigName, Vec<SnapshotInfo>)>, SnapshotError> {
    let mut configs = backend.list_configs()?;
    configs.sort();
    configs.dedup();
    configs
        .into_iter()
        .map(|config| {
            let snapshots = backend.list_snapshots(&config)?;
            Ok((config, snapshots))
        })
        .collect()
}

/// Delete one snapshot of an existing config.
pub fn delete_snapshot(
    backend: &dyn SnapshotBackend,
    config: &ConfigName,
    id: SnapshotId,
) -> Result<(), SnapshotError> {
    ensure_config(backend, config)?;
    Ok(backend.delete_snapshot(config, id)?)
}

/// Why a restore was refused or failed.
#[derive(Debug, Error)]
pub enum RestoreError {
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("{config} has no snapshot #{id}")]
    UnknownSnapshot { config: ConfigName, id: SnapshotId },
    /// Putting the whole system root back while it is running would leave files, kernel and
    /// boot menu out of step with each other. Single paths are fine; the system itself is
    /// restored from the boot menu or a live system.
    #[error("`{config}` is the system root: restoring all of it while it runs is not safe (restore single paths, or restore the system from the boot menu)")]
    SystemRoot { config: ConfigName },
    #[error("`{path}` is not an absolute path without `..`")]
    BadPath { path: String },
    #[error("`{path}` is outside {config}, which is {subvolume}")]
    OutsideConfig {
        config: ConfigName,
        path: String,
        subvolume: String,
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
    pub subvolume: String,
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

fn is_root(subvolume: &str) -> bool {
    subvolume.trim_end_matches('/').is_empty()
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
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<String, RestoreError> {
    ensure_config(backend, config)?;
    if !backend.list_snapshots(config)?.iter().any(|s| s.id == id) {
        return Err(RestoreError::UnknownSnapshot {
            config: config.clone(),
            id,
        });
    }
    let subvolume = backend.config_subvolume(config)?;
    if let Some(path) = path {
        if !clean(path) {
            return Err(RestoreError::BadPath {
                path: path.to_owned(),
            });
        }
        if !under(&subvolume, path) {
            return Err(RestoreError::OutsideConfig {
                config: config.clone(),
                path: path.to_owned(),
                subvolume,
            });
        }
    }
    Ok(subvolume)
}

/// What restoring snapshot `id` of `config` would change. Read-only.
pub fn preview_restore(
    backend: &dyn SnapshotBackend,
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<Preview, RestoreError> {
    let subvolume = checked(backend, config, id, path)?;
    let mut changes = backend.changes_since(config, id)?;
    if let Some(path) = path {
        changes.retain(|change| under(path, &change.path));
    }
    Ok(Preview {
        is_system_root: is_root(&subvolume),
        subvolume,
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
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<Restored, RestoreError> {
    let subvolume = checked(backend, config, id, path)?;
    if path.is_none() && is_root(&subvolume) {
        return Err(RestoreError::SystemRoot {
            config: config.clone(),
        });
    }
    let note = Description::parse(&format!("before restoring #{id}")).map_err(|_| {
        RestoreError::BadPath {
            path: format!("#{id}"),
        }
    })?;
    let safety_snapshot = backend
        .create_snapshot(config, &note)
        .map_err(RestoreError::SafetySnapshot)?;
    let summary = backend.undo_changes(config, id, path)?;
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
    #[error("no snapper config covers `{path}`: it is not under any config's subvolume")]
    NoConfig { path: String },
    /// The path is under a config's directory but on another subvolume, which that config's
    /// snapshots do not include. Saying "did not exist" here would be wrong: it never could have.
    #[error("`{path}` is on a different subvolume than config `{config}` (`{subvolume}`), so no snapshot of `{config}` contains it")]
    OtherSubvolume {
        path: String,
        config: ConfigName,
        subvolume: String,
    },
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// A file, the config that covers it, and how it stood in each snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistory {
    pub config: ConfigName,
    pub path: String,
    /// Oldest first, as snapper lists them.
    pub versions: Vec<FileVersion>,
}

/// The config whose subvolume holds `path`: the one with the longest matching mount point, so a
/// file in `/home` belongs to `home` and not to `root`, which also contains it.
pub fn config_covering(
    backend: &dyn SnapshotBackend,
    path: &str,
) -> Result<ConfigName, HistoryError> {
    if !clean(path) {
        return Err(HistoryError::BadPath {
            path: path.to_owned(),
        });
    }
    // The most specific config whose subvolume really holds the file (by length of mount point),
    // and, failing that, the most specific one that merely has it under its directory.
    let mut covering: Option<(usize, ConfigName)> = None;
    let mut only_by_name: Option<(usize, ConfigName, String)> = None;
    for config in backend.list_configs().map_err(SnapshotError::from)? {
        let subvolume = backend.config_subvolume(&config)?;
        if !under(&subvolume, path) {
            continue;
        }
        let length = subvolume.trim_end_matches('/').len();
        if backend.covers(&config, path)? {
            if covering
                .as_ref()
                .is_none_or(|(longest, _)| length > *longest)
            {
                covering = Some((length, config));
            }
        } else if only_by_name
            .as_ref()
            .is_none_or(|(longest, ..)| length > *longest)
        {
            only_by_name = Some((length, config, subvolume));
        }
    }
    match (covering, only_by_name) {
        (Some((_, config)), _) => Ok(config),
        (None, Some((_, config, subvolume))) => Err(HistoryError::OtherSubvolume {
            path: path.to_owned(),
            config,
            subvolume,
        }),
        (None, None) => Err(HistoryError::NoConfig {
            path: path.to_owned(),
        }),
    }
}

/// How `path` stood in every snapshot of `config`.
pub fn file_versions_of(
    backend: &dyn SnapshotBackend,
    config: &ConfigName,
    path: &str,
) -> Result<Vec<FileVersion>, HistoryError> {
    if !clean(path) {
        return Err(HistoryError::BadPath {
            path: path.to_owned(),
        });
    }
    ensure_config(backend, config)?;
    let subvolume = backend.config_subvolume(config)?;
    if !under(&subvolume, path) {
        return Err(HistoryError::NoConfig {
            path: path.to_owned(),
        });
    }
    if !backend.covers(config, path)? {
        return Err(HistoryError::OtherSubvolume {
            path: path.to_owned(),
            config: config.clone(),
            subvolume,
        });
    }
    Ok(backend.file_versions(config, path)?)
}

/// The history of one file: which config covers it, and how it stood in each snapshot.
pub fn file_history(
    backend: &dyn SnapshotBackend,
    path: &str,
) -> Result<FileHistory, HistoryError> {
    let config = config_covering(backend, path)?;
    let versions = file_versions_of(backend, &config, path)?;
    Ok(FileHistory {
        config,
        path: path.to_owned(),
        versions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::InMemoryBackend;

    fn desc() -> Description {
        Description::parse("test").unwrap()
    }

    #[test]
    fn snapshots_every_config_by_default() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        let report = snapshot_all(&backend, None, &desc()).unwrap();
        assert!(report.is_success());
        assert_eq!(report.outcomes.len(), 2);
        assert_eq!(backend.snapshots().len(), 2);
    }

    #[test]
    fn only_restricts_the_targets() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        let only = [ConfigName::parse("home").unwrap()];
        let report = snapshot_all(&backend, Some(&only), &desc()).unwrap();
        assert_eq!(report.outcomes.len(), 1);
        assert_eq!(backend.snapshots()[0].0.as_str(), "home");
    }

    #[test]
    fn one_failure_does_not_stop_the_rest_and_is_reported() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]).failing_on("home");
        let report = snapshot_all(&backend, None, &desc()).unwrap();
        assert_eq!(report.outcomes.len(), 2);
        assert_eq!(report.failures(), 1);
        assert!(!report.is_success());
        assert_eq!(backend.snapshots().len(), 1);
    }

    #[test]
    fn planning_alone_takes_no_snapshot() {
        let backend = InMemoryBackend::with_configs(&["root"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        assert_eq!(plan.targets().len(), 1);
        assert!(backend.snapshots().is_empty());
    }

    #[test]
    fn a_backend_without_configs_is_a_plan_error() {
        let backend = InMemoryBackend::with_configs(&[]);
        assert!(matches!(
            snapshot_all(&backend, None, &desc()),
            Err(SnapshotError::Plan(PlanError::NoConfigs))
        ));
    }

    #[test]
    fn listing_and_deleting_go_through_the_use_cases() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        snapshot_all(&backend, None, &desc()).unwrap();
        let root = ConfigName::parse("root").unwrap();

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
    fn an_unknown_config_is_our_error_not_the_backends() {
        let backend = InMemoryBackend::with_configs(&["root"]);
        let nope = ConfigName::parse("nope").unwrap();
        assert!(matches!(
            snapshots_of(&backend, &nope),
            Err(SnapshotError::Plan(PlanError::UnknownConfigs(_)))
        ));
        assert!(matches!(
            delete_snapshot(&backend, &nope, SnapshotId::new(1).unwrap()),
            Err(SnapshotError::Plan(PlanError::UnknownConfigs(_)))
        ));
    }

    // ---- restoring -----------------------------------------------------------------------

    use crate::domain::ChangeKind;

    fn home() -> ConfigName {
        ConfigName::parse("home").unwrap()
    }

    fn id(n: u64) -> SnapshotId {
        SnapshotId::new(n).unwrap()
    }

    /// `home` with a.txt = "one", snapshotted as #1, then a.txt edited and b.txt added.
    fn edited_home() -> InMemoryBackend {
        let backend = InMemoryBackend::with_configs(&["home"]);
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
        let backend = InMemoryBackend::with_configs(&["root"]).with_root_config("root");
        let root = ConfigName::parse("root").unwrap();
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
    fn paths_cannot_escape_the_config_and_a_refusal_changes_nothing() {
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
                matches!(error, RestoreError::OutsideConfig { .. }),
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
    fn an_unknown_snapshot_or_config_is_refused_before_anything_changes() {
        let backend = edited_home();
        assert!(matches!(
            restore_snapshot(&backend, &home(), id(9), None),
            Err(RestoreError::UnknownSnapshot { .. })
        ));
        let nope = ConfigName::parse("nope").unwrap();
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
    fn the_config_with_the_longest_mount_point_covers_a_file() {
        let backend = InMemoryBackend::with_configs(&["home", "root"]).with_root_config("root");
        assert_eq!(
            config_covering(&backend, "/mnt/home/a.txt")
                .unwrap()
                .as_str(),
            "home"
        );
        assert_eq!(
            config_covering(&backend, "/etc/fstab").unwrap().as_str(),
            "root"
        );
        // a sibling that merely starts with the same letters is not inside
        assert_eq!(
            config_covering(&backend, "/mnt/homeless/a.txt")
                .unwrap()
                .as_str(),
            "root"
        );
    }

    #[test]
    fn a_file_no_config_covers_or_a_path_that_could_escape_is_refused() {
        let backend = InMemoryBackend::with_configs(&["home"]);
        assert!(matches!(
            config_covering(&backend, "/etc/fstab"),
            Err(HistoryError::NoConfig { .. })
        ));
        for bad in ["relative.txt", "/mnt/home/../etc/passwd", "/mnt/home/a\nb"] {
            assert!(
                matches!(
                    config_covering(&backend, bad),
                    Err(HistoryError::BadPath { .. })
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_files_history_says_in_each_snapshot_how_it_compares_with_now() {
        let backend = InMemoryBackend::with_configs(&["home"]);
        backend.write_file("home", "/mnt/home/a.txt", "one");
        backend.create_snapshot(&home(), &desc()).unwrap(); // #1: one
        backend.write_file("home", "/mnt/home/a.txt", "two");
        backend.create_snapshot(&home(), &desc()).unwrap(); // #2: two
        backend.write_file("home", "/mnt/home/b.txt", "new"); // not in either snapshot

        let a = file_history(&backend, "/mnt/home/a.txt").unwrap();
        assert_eq!(a.config, home());
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
    fn a_history_for_a_path_outside_the_given_config_is_refused() {
        let backend = InMemoryBackend::with_configs(&["home", "root"]).with_root_config("root");
        assert!(matches!(
            file_versions_of(&backend, &home(), "/etc/fstab"),
            Err(HistoryError::NoConfig { .. })
        ));
    }

    // ---- what a snapshot cannot contain ---------------------------------------------------------

    #[test]
    fn a_file_on_a_separate_subvolume_is_not_attributed_to_the_config_above_it() {
        // root is `/`, and /mnt/home is a separate subvolume with no config of its own
        let backend = InMemoryBackend::with_configs(&["root"])
            .with_root_config("root")
            .with_separate_subvolume("/mnt/home");
        let error = config_covering(&backend, "/mnt/home/u/notes.txt").unwrap_err();
        assert!(
            matches!(&error, HistoryError::OtherSubvolume { config, .. } if config.as_str() == "root"),
            "{error}"
        );
        assert!(
            error
                .to_string()
                .contains("no snapshot of `root` contains it"),
            "{error}"
        );
        // a file of the root subvolume itself is still covered by it
        assert_eq!(
            config_covering(&backend, "/etc/fstab").unwrap().as_str(),
            "root"
        );
        // and asking for the history through the config directly says the same, not "did not exist"
        let direct = file_versions_of(
            &backend,
            &ConfigName::parse("root").unwrap(),
            "/mnt/home/u/notes.txt",
        );
        assert!(
            matches!(direct, Err(HistoryError::OtherSubvolume { .. })),
            "{direct:?}"
        );
    }

    #[test]
    fn once_the_separate_subvolume_has_its_own_config_that_config_covers_its_files() {
        let backend = InMemoryBackend::with_configs(&["home", "root"])
            .with_root_config("root")
            .with_separate_subvolume("/mnt/home");
        assert_eq!(
            config_covering(&backend, "/mnt/home/u/notes.txt")
                .unwrap()
                .as_str(),
            "home"
        );
        assert_eq!(
            config_covering(&backend, "/etc/fstab").unwrap().as_str(),
            "root"
        );
    }
}
