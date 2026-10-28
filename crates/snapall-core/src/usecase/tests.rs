// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The use cases, against in-memory stand-ins for the port.

#![cfg(test)]

use crate::domain::{AreaName, Description, SnapshotId};
use crate::plan::PlanError;
use crate::port::SnapshotBackend;

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
