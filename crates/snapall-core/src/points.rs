// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restore points: the snapshots that one action took together, one in every area, found again and
//! put back with a single action.
//!
//! A point is found by its tag. Snapshots from before the tag existed are matched instead: the same
//! description in different areas, taken within two minutes of each other, which is exactly what one
//! `snapall` did. A matched point is marked as a guess, so nobody mistakes it for a fact.
//!
//! The system itself is never put back while it runs (see ADR 12): restoring a point puts back every
//! other area, and says so plainly about the system.

use std::collections::BTreeMap;

use crate::calendar::{compact_utc, seconds_from_snapper_date};
use crate::course::{Course, Unattended};
use crate::domain::{ConfigName, Description, Point, SnapshotId, SnapshotInfo, SnapshotKind};
use crate::port::SnapshotBackend;
use crate::usecase::{preview_restore, restore_snapshot_with, Preview, Restored, Safety};

/// How far apart, in seconds, snapshots of different areas can be and still count as one action's.
const MATCH_WINDOW_SECONDS: i64 = 120;

/// A name for the restore point of an action that starts at `seconds`: the time, and if a point of
/// that name is already in use (two actions in one second) a `-2`, `-3`, ... after it. Two points
/// sharing a name would be one point, so a restore's safety snapshots could not be told from what
/// they protect. If the existing points cannot be read, the plain time is used.
pub fn fresh_point(backend: &dyn SnapshotBackend, seconds: u64) -> Point {
    let taken: std::collections::BTreeSet<String> = crate::usecase::all_snapshots(backend)
        .map(|all| {
            all.iter()
                .flat_map(|(_, snapshots)| snapshots.iter())
                .filter_map(|snapshot| snapshot.point.as_ref().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    let base = compact_utc(seconds);
    std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .find(|candidate| !taken.contains(candidate))
        .and_then(|candidate| Point::parse(&candidate).ok())
        .unwrap_or_else(|| Point::at(seconds))
}

/// One area's snapshot in a restore point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub config: ConfigName,
    pub id: SnapshotId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePoint {
    pub point: Point,
    /// What it was called when it was taken.
    pub description: String,
    /// When the earliest of its snapshots was taken, as snapper wrote it.
    pub date: Option<String>,
    /// One snapshot per area, in the order of the areas' names.
    pub members: Vec<Member>,
    /// `true` if found by its tag, `false` if matched by description and time.
    pub tagged: bool,
}

type Listing<'a> = &'a [(ConfigName, Vec<SnapshotInfo>)];

/// The restore points among these snapshots, newest first. A point needs snapshots in at least two
/// areas: with one there is nothing to put back together.
pub fn restore_points(all: Listing<'_>) -> Vec<RestorePoint> {
    let mut found = tagged_points(all);
    found.extend(matched_points(all));
    found.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.point.cmp(&a.point)));
    found
}

/// The restore point a snapshot belongs to, if any.
pub fn point_of<'a>(
    points: &'a [RestorePoint],
    config: &ConfigName,
    id: SnapshotId,
) -> Option<&'a RestorePoint> {
    points.iter().find(|point| {
        point
            .members
            .iter()
            .any(|m| m.config == *config && m.id == id)
    })
}

fn tagged_points(all: Listing<'_>) -> Vec<RestorePoint> {
    let mut by_tag: BTreeMap<&Point, Vec<(&ConfigName, &SnapshotInfo)>> = BTreeMap::new();
    for (config, snapshots) in all {
        for snapshot in snapshots {
            if let Some(point) = &snapshot.point {
                by_tag.entry(point).or_default().push((config, snapshot));
            }
        }
    }
    by_tag
        .into_iter()
        .filter(|(_, rows)| distinct_areas(rows) >= 2)
        .map(|(point, rows)| build(point.clone(), &rows, true))
        .collect()
}

fn matched_points(all: Listing<'_>) -> Vec<RestorePoint> {
    // Snapshots with no tag, taken one at a time, whose date can be read.
    let mut by_description: BTreeMap<&str, Vec<(i64, &ConfigName, &SnapshotInfo)>> =
        BTreeMap::new();
    for (config, snapshots) in all {
        for snapshot in snapshots {
            let seconds = snapshot.date.as_deref().and_then(seconds_from_snapper_date);
            if let (None, SnapshotKind::Single, Some(seconds)) =
                (&snapshot.point, &snapshot.kind, seconds)
            {
                by_description
                    .entry(snapshot.description.as_str())
                    .or_default()
                    .push((seconds, config, snapshot));
            }
        }
    }
    let mut points = Vec::new();
    for rows in by_description.values_mut() {
        rows.sort_by_key(|(seconds, ..)| *seconds);
        let mut cluster: Vec<(i64, &ConfigName, &SnapshotInfo)> = Vec::new();
        for row in rows
            .iter()
            .copied()
            .chain(std::iter::once((i64::MAX, rows[0].1, rows[0].2)))
        {
            let continues = cluster.first().is_some_and(|first| {
                row.0 != i64::MAX
                    && row.0 - first.0 <= MATCH_WINDOW_SECONDS
                    && cluster.iter().all(|(_, config, _)| *config != row.1)
            });
            if continues {
                cluster.push(row);
                continue;
            }
            if cluster.len() >= 2 {
                let first = cluster[0].0;
                let name = format!("matched-{}", compact_utc(u64::try_from(first).unwrap_or(0)));
                if let Ok(point) = Point::parse(&name) {
                    let rows: Vec<(&ConfigName, &SnapshotInfo)> =
                        cluster.iter().map(|(_, c, s)| (*c, *s)).collect();
                    points.push(build(point, &rows, false));
                }
            }
            cluster.clear();
            if row.0 != i64::MAX {
                cluster.push(row);
            }
        }
    }
    points
}

fn distinct_areas(rows: &[(&ConfigName, &SnapshotInfo)]) -> usize {
    let mut names: Vec<&ConfigName> = rows.iter().map(|(config, _)| *config).collect();
    names.sort();
    names.dedup();
    names.len()
}

fn build(point: Point, rows: &[(&ConfigName, &SnapshotInfo)], tagged: bool) -> RestorePoint {
    let mut members: Vec<Member> = rows
        .iter()
        .map(|(config, snapshot)| Member {
            config: (*config).clone(),
            id: snapshot.id,
        })
        .collect();
    members.sort_by(|a, b| a.config.cmp(&b.config));
    RestorePoint {
        point,
        description: rows
            .first()
            .map(|(_, snapshot)| snapshot.description.clone())
            .unwrap_or_default(),
        date: rows.iter().filter_map(|(_, s)| s.date.clone()).min(),
        members,
        tagged,
    }
}

/// What restoring one area of a point would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberView {
    /// These paths would change.
    Changes(Preview),
    /// The system itself: not put back while it runs.
    SystemRoot,
    /// It could not be looked at.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberPreview {
    pub config: ConfigName,
    pub id: SnapshotId,
    pub view: MemberView,
}

/// What restoring `point` would do to each of its areas, changing nothing.
pub fn preview_point(backend: &dyn SnapshotBackend, point: &RestorePoint) -> Vec<MemberPreview> {
    point
        .members
        .iter()
        .map(|member| {
            let view = match preview_restore(backend, &member.config, member.id, None) {
                Ok(preview) if preview.is_system_root => MemberView::SystemRoot,
                Ok(preview) => MemberView::Changes(preview),
                Err(error) => MemberView::Failed(error.to_string()),
            };
            MemberPreview {
                config: member.config.clone(),
                id: member.id,
                view,
            }
        })
        .collect()
}

#[derive(Debug)]
pub enum MemberResult {
    Restored(Restored),
    /// It already matched: nothing to put back, and no safety snapshot was needed.
    AlreadyThere,
    /// The system itself: left alone.
    SystemRoot,
    Failed(String),
}

#[derive(Debug)]
pub struct MemberReport {
    pub config: ConfigName,
    pub id: SnapshotId,
    pub result: MemberResult,
}

/// What restoring a point did.
#[derive(Debug)]
pub struct PointRestore {
    /// The restore point of the snapshots taken just before: restoring *it* undoes all of this.
    pub safety: Point,
    pub members: Vec<MemberReport>,
    /// It was stopped, on request, before every area had its turn: the areas in `members` are done and
    /// the rest were left as they are.
    pub stopped: bool,
}

impl PointRestore {
    pub fn succeeded(&self) -> bool {
        !self
            .members
            .iter()
            .any(|m| matches!(m.result, MemberResult::Failed(_)))
    }

    /// Whether any area was actually put back, so that there is something to undo.
    pub fn changed_anything(&self) -> bool {
        self.members
            .iter()
            .any(|m| matches!(m.result, MemberResult::Restored(_)))
    }
}

/// Put every area of `point` back to how it was, one after another. Each area takes its own safety
/// snapshot first, all tagged as the restore point `safety`. One area failing does not stop the
/// others (and every outcome is reported): they are independent, and the safety snapshots make the
/// whole thing undoable either way.
pub fn restore_point(
    backend: &dyn SnapshotBackend,
    point: &RestorePoint,
    safety: Point,
) -> PointRestore {
    restore_point_along(backend, point, safety, &Unattended)
}

/// [`restore_point`] following a [`Course`]: told before each area, and stoppable between them (never in
/// the middle of one). Stopped, the areas already put back stay put back, and `stopped` says so.
pub fn restore_point_along(
    backend: &dyn SnapshotBackend,
    point: &RestorePoint,
    safety: Point,
    course: &dyn Course,
) -> PointRestore {
    let note = Description::parse(&format!("before restoring {}", point.point)).unwrap_or_default();
    let tagging = Safety {
        point: safety.clone(),
        note,
    };
    let total = point.members.len();
    let mut members = Vec::with_capacity(total);
    let mut stopped = false;
    for (done, member) in point.members.iter().enumerate() {
        if !course.proceed(done, total, member.config.as_str()) {
            stopped = true;
            break;
        }
        let result = match preview_restore(backend, &member.config, member.id, None) {
            Err(error) => MemberResult::Failed(error.to_string()),
            Ok(preview) if preview.is_system_root => MemberResult::SystemRoot,
            Ok(preview) if preview.changes.is_empty() => MemberResult::AlreadyThere,
            Ok(_) => {
                match restore_snapshot_with(
                    backend,
                    &member.config,
                    member.id,
                    None,
                    Some(&tagging),
                ) {
                    Ok(restored) => MemberResult::Restored(restored),
                    Err(error) => MemberResult::Failed(error.to_string()),
                }
            }
        };
        members.push(MemberReport {
            config: member.config.clone(),
            id: member.id,
            result,
        });
    }
    PointRestore {
        safety,
        members,
        stopped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::InMemoryBackend;
    use crate::usecase::{plan_snapshots, run_plan};

    fn name(s: &str) -> ConfigName {
        ConfigName::parse(s).unwrap()
    }
    fn info(id: u64, description: &str, date: &str, point: Option<&str>) -> SnapshotInfo {
        SnapshotInfo {
            id: SnapshotId::new(id).unwrap(),
            kind: SnapshotKind::Single,
            date: Some(date.to_owned()),
            user: "root".into(),
            cleanup: Some("number".into()),
            description: description.into(),
            point: point.map(|p| Point::parse(p).unwrap()),
            used_bytes: None,
        }
    }

    // ---- finding points -------------------------------------------------------------------------

    #[test]
    fn snapshots_with_the_same_tag_in_different_areas_are_one_point_with_one_snapshot_per_area() {
        let all = vec![
            (
                name("home"),
                vec![
                    info(3, "before theming", "2026-10-25 18:00:01", Some("p1")),
                    info(4, "x", "2026-10-25 19:00:00", None),
                ],
            ),
            (
                name("root"),
                vec![info(9, "before theming", "2026-10-25 18:00:02", Some("p1"))],
            ),
        ];
        let points = restore_points(&all);
        assert_eq!(points.len(), 1);
        let point = &points[0];
        assert!(
            point.tagged && point.point.as_str() == "p1" && point.description == "before theming"
        );
        assert_eq!(
            point.date.as_deref(),
            Some("2026-10-25 18:00:01"),
            "the earliest of its snapshots"
        );
        let members: Vec<(&str, u64)> = point
            .members
            .iter()
            .map(|m| (m.config.as_str(), m.id.get()))
            .collect();
        assert_eq!(members, [("home", 3), ("root", 9)]);
    }

    #[test]
    fn a_tag_in_only_one_area_is_not_a_point_there_is_nothing_to_put_back_together() {
        let all = vec![
            (
                name("home"),
                vec![info(3, "a", "2026-10-25 18:00:01", Some("p1"))],
            ),
            (
                name("root"),
                vec![info(9, "a", "2026-10-25 18:00:02", None)],
            ),
        ];
        // the untagged one is matched with it? no: a tagged snapshot is never matched by guesswork
        assert!(restore_points(&all).is_empty());
    }

    #[test]
    fn older_untagged_snapshots_are_matched_by_description_and_time_and_marked_as_a_guess() {
        let all = vec![
            (
                name("home"),
                vec![info(1, "before theming", "2026-10-25 18:00:00", None)],
            ),
            (
                name("root"),
                vec![info(5, "before theming", "2026-10-25 18:00:40", None)],
            ),
        ];
        let points = restore_points(&all);
        assert_eq!(points.len(), 1);
        assert!(!points[0].tagged, "matched, not tagged");
        assert!(
            points[0].point.as_str().starts_with("matched-2026"),
            "{}",
            points[0].point
        );
        assert_eq!(points[0].members.len(), 2);
    }

    #[test]
    fn matching_needs_the_same_words_and_the_same_few_minutes_and_one_snapshot_per_area() {
        let all = vec![
            (
                name("home"),
                vec![
                    info(1, "before theming", "2026-10-25 18:00:00", None),
                    info(2, "before theming", "2026-10-25 18:00:30", None), // same area again: a new group
                    info(3, "snapall", "2026-10-25 18:00:00", None),
                ],
            ),
            (
                name("root"),
                vec![
                    info(5, "before theming", "2026-10-25 18:00:20", None),
                    info(6, "snapall", "2026-10-25 20:00:00", None), // hours later: not the same action
                ],
            ),
        ];
        let points = restore_points(&all);
        assert_eq!(points.len(), 1, "{points:#?}");
        let members: Vec<(&str, u64)> = points[0]
            .members
            .iter()
            .map(|m| (m.config.as_str(), m.id.get()))
            .collect();
        assert_eq!(
            members,
            [("home", 1), ("root", 5)],
            "the first home snapshot pairs with root's; the second has no partner"
        );
    }

    #[test]
    fn points_come_newest_first_and_a_snapshot_finds_its_point() {
        let all = vec![
            (
                name("home"),
                vec![
                    info(1, "old", "2026-10-24 10:00:00", Some("a")),
                    info(2, "new", "2026-10-25 10:00:00", Some("b")),
                ],
            ),
            (
                name("root"),
                vec![
                    info(7, "old", "2026-10-24 10:00:01", Some("a")),
                    info(8, "new", "2026-10-25 10:00:01", Some("b")),
                ],
            ),
        ];
        let points = restore_points(&all);
        assert_eq!(
            points.iter().map(|p| p.point.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        let found = point_of(&points, &name("root"), SnapshotId::new(7).unwrap()).unwrap();
        assert_eq!(found.point.as_str(), "a");
        assert!(point_of(&points, &name("root"), SnapshotId::new(99).unwrap()).is_none());
    }

    // ---- restoring a point ----------------------------------------------------------------------

    /// docs and home are ordinary areas, root is the system; one action snapshots all three.
    fn three_areas() -> (InMemoryBackend, RestorePoint) {
        let backend =
            InMemoryBackend::with_configs(&["docs", "home", "root"]).with_root_config("root");
        backend.write_file("docs", "/mnt/docs/a.txt", "docs, then");
        backend.write_file("home", "/mnt/home/b.txt", "home, then");
        let plan = plan_snapshots(&backend, None).unwrap();
        let point = Point::parse("20261025T100000Z").unwrap();
        run_plan(
            &backend,
            &plan,
            &Description::parse("before theming").unwrap(),
            Some(&point),
        );
        let all = crate::usecase::all_snapshots(&backend).unwrap();
        let points = restore_points(&all);
        assert_eq!(points.len(), 1, "{points:#?}");
        (backend, points.into_iter().next().unwrap())
    }

    #[test]
    fn restoring_a_point_puts_every_ordinary_area_back_and_leaves_the_system_alone() {
        let (backend, point) = three_areas();
        backend.write_file("docs", "/mnt/docs/a.txt", "docs, ruined");
        backend.write_file("home", "/mnt/home/b.txt", "home, ruined");
        backend.write_file("root", "/etc/x", "system, changed");

        let done = restore_point(&backend, &point, Point::parse("20261025T110000Z").unwrap());

        assert!(done.succeeded() && done.changed_anything());
        assert_eq!(
            backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
            Some("docs, then")
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("home, then")
        );
        assert_eq!(
            backend.read_file("root", "/etc/x").as_deref(),
            Some("system, changed"),
            "the running system is not touched"
        );
        let by: BTreeMap<&str, &MemberResult> = done
            .members
            .iter()
            .map(|m| (m.config.as_str(), &m.result))
            .collect();
        assert!(
            matches!(by["docs"], MemberResult::Restored(_))
                && matches!(by["home"], MemberResult::Restored(_))
        );
        assert!(matches!(by["root"], MemberResult::SystemRoot));
    }

    #[test]
    fn the_safety_snapshots_are_one_restore_point_so_undoing_everything_is_one_action() {
        let (backend, point) = three_areas();
        backend.write_file("docs", "/mnt/docs/a.txt", "docs, ruined");
        backend.write_file("home", "/mnt/home/b.txt", "home, ruined");
        let safety = Point::parse("20261025T110000Z").unwrap();
        restore_point(&backend, &point, safety.clone());
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("home, then")
        );

        // the safety snapshots form a point of their own, named for what they are
        let all = crate::usecase::all_snapshots(&backend).unwrap();
        let undo = restore_points(&all)
            .into_iter()
            .find(|p| p.point == safety)
            .expect("the safety point is listed");
        assert_eq!(undo.description, "before restoring 20261025T100000Z");
        assert_eq!(
            undo.members
                .iter()
                .map(|m| m.config.as_str())
                .collect::<Vec<_>>(),
            ["docs", "home"],
            "only the areas that changed took one"
        );

        restore_point(&backend, &undo, Point::parse("20261025T120000Z").unwrap());
        assert_eq!(
            backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
            Some("docs, ruined"),
            "back to just before"
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("home, ruined")
        );
    }

    #[test]
    fn an_area_that_already_matches_is_left_alone_and_takes_no_safety_snapshot() {
        let (backend, point) = three_areas();
        backend.write_file("home", "/mnt/home/b.txt", "home, ruined"); // only home changed
        let before = backend.snapshots().len();
        let done = restore_point(&backend, &point, Point::parse("20261025T110000Z").unwrap());
        let by: BTreeMap<&str, &MemberResult> = done
            .members
            .iter()
            .map(|m| (m.config.as_str(), &m.result))
            .collect();
        assert!(matches!(by["docs"], MemberResult::AlreadyThere));
        assert!(matches!(by["home"], MemberResult::Restored(_)));
        assert_eq!(
            backend.snapshots().len(),
            before + 1,
            "one safety snapshot, for the one area that changed"
        );
    }

    #[test]
    fn one_area_failing_is_reported_and_does_not_stop_the_others() {
        let (backend, point) = three_areas();
        backend.write_file("docs", "/mnt/docs/a.txt", "docs, ruined");
        backend.write_file("home", "/mnt/home/b.txt", "home, ruined");
        backend.refuse_snapshots_of("docs"); // its safety snapshot cannot be taken, so it is not restored
        let done = restore_point(&backend, &point, Point::parse("20261025T110000Z").unwrap());
        assert!(!done.succeeded());
        let by: BTreeMap<&str, &MemberResult> = done
            .members
            .iter()
            .map(|m| (m.config.as_str(), &m.result))
            .collect();
        assert!(matches!(by["docs"], MemberResult::Failed(_)));
        assert!(
            matches!(by["home"], MemberResult::Restored(_)),
            "home was still put back"
        );
        assert_eq!(
            backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
            Some("docs, ruined"),
            "a failed safety snapshot means nothing was changed"
        );
    }

    #[test]
    fn two_actions_in_the_same_second_get_different_points_so_a_safety_net_is_never_its_own_target()
    {
        let backend = InMemoryBackend::with_configs(&["docs", "home"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        let note = Description::parse("x").unwrap();
        let mut names = Vec::new();
        for _ in 0..3 {
            let point = fresh_point(&backend, 1_700_000_000);
            run_plan(&backend, &plan, &note, Some(&point));
            names.push(point.to_string());
        }
        assert_eq!(
            names,
            [
                "20231114T221320Z",
                "20231114T221320Z-2",
                "20231114T221320Z-3"
            ]
        );
        let all = crate::usecase::all_snapshots(&backend).unwrap();
        assert_eq!(
            restore_points(&all).len(),
            3,
            "three separate points, not one merged one"
        );
        // a different second is untouched by the earlier ones
        assert_eq!(
            fresh_point(&backend, 1_700_000_001).to_string(),
            "20231114T221321Z"
        );
    }

    #[test]
    fn the_preview_says_what_would_change_per_area_and_changes_nothing() {
        let (backend, point) = three_areas();
        backend.write_file("home", "/mnt/home/b.txt", "home, ruined");
        let before = backend.snapshots().len();
        let views = preview_point(&backend, &point);
        let by: BTreeMap<&str, &MemberView> =
            views.iter().map(|v| (v.config.as_str(), &v.view)).collect();
        assert!(matches!(by["root"], MemberView::SystemRoot));
        assert!(matches!(by["home"], MemberView::Changes(p) if p.changes.len() == 1));
        assert!(matches!(by["docs"], MemberView::Changes(p) if p.changes.is_empty()));
        assert_eq!(backend.snapshots().len(), before, "looking changes nothing");
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("home, ruined")
        );
    }
}
