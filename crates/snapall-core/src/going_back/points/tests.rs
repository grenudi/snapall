// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restore points, found and put back, against an in-memory backend.

#![cfg(test)]

use crate::vocabulary::domain::{
    AreaName, Description, Point, SnapshotId, SnapshotInfo, SnapshotKind,
};
use std::collections::BTreeMap;

use super::*;
use crate::ports::testing::InMemoryBackend;
use crate::taking::run::{plan_snapshots, run_plan};

fn name(s: &str) -> AreaName {
    AreaName::parse(s).unwrap()
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
    assert!(point.tagged && point.point.as_str() == "p1" && point.description == "before theming");
    assert_eq!(
        point.date.as_deref(),
        Some("2026-10-25 18:00:01"),
        "the earliest of its snapshots"
    );
    let members: Vec<(&str, u64)> = point
        .members
        .iter()
        .map(|m| (m.area.as_str(), m.id.get()))
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
        .map(|m| (m.area.as_str(), m.id.get()))
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
    let backend = InMemoryBackend::with_areas(&["docs", "home", "root"]).with_root_area("root");
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
    let all = crate::looking::snapshots::all_snapshots(&backend).unwrap();
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
        .map(|m| (m.area.as_str(), &m.result))
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
    let all = crate::looking::snapshots::all_snapshots(&backend).unwrap();
    let undo = restore_points(&all)
        .into_iter()
        .find(|p| p.point == safety)
        .expect("the safety point is listed");
    assert_eq!(undo.description, "before restoring 20261025T100000Z");
    assert_eq!(
        undo.members
            .iter()
            .map(|m| m.area.as_str())
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
        .map(|m| (m.area.as_str(), &m.result))
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
        .map(|m| (m.area.as_str(), &m.result))
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
fn two_actions_in_the_same_second_get_different_points_so_a_safety_net_is_never_its_own_target() {
    let backend = InMemoryBackend::with_areas(&["docs", "home"]);
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
    let all = crate::looking::snapshots::all_snapshots(&backend).unwrap();
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
        views.iter().map(|v| (v.area.as_str(), &v.view)).collect();
    assert!(matches!(by["root"], MemberView::SystemRoot));
    assert!(matches!(by["home"], MemberView::Changes(p) if p.changes.len() == 1));
    assert!(matches!(by["docs"], MemberView::Changes(p) if p.changes.is_empty()));
    assert_eq!(backend.snapshots().len(), before, "looking changes nothing");
    assert_eq!(
        backend.read_file("home", "/mnt/home/b.txt").as_deref(),
        Some("home, ruined")
    );
}
