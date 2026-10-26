// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The port contract, run against a real snapper on a real Btrfs volume.
//!
//! Skipped unless `SNAPALL_IT=1`. CI sets it up with `dev/btrfs-fixture.sh up` (root,
//! loop devices, snapper and btrfs-progs required).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use snapall_core::testing::backend_contract;
use snapall_core::{
    all_snapshots, config_covering, file_history, fresh_point, preview_restore, protect_area,
    restore_point, restore_points, restore_snapshot, snapshot_all, ChangeKind, ConfigName,
    Description, FileState, HistoryError, MemberResult, Point, RestoreError, SetupError,
    SnapshotBackend,
};
use snapall_snapper::SnapperCli;

/// These tests share one filesystem and one list of configs. A test that has a config for a moment
/// (setting an area up) would be seen, and snapshotted, by the test that snapshots every config, and a
/// snapshot made while a config is being removed makes the removal fail. They take turns.
static TAKE_TURNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn take_turns() -> std::sync::MutexGuard<'static, ()> {
    // a test that failed must not stop the others from running
    TAKE_TURNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Removes what a test made, even when the test fails half way: what is left behind fails every
/// later run too (`target path already exists`), and hides the first failure under its own.
struct Cleanup {
    config: Option<String>,
    subvolumes: Vec<String>,
    directories: Vec<String>,
}

impl Cleanup {
    fn run(&self) {
        use std::process::Command;
        if let Some(config) = &self.config {
            let _ = Command::new("snapper")
                .args(["--no-dbus", "-c", config, "delete-config"])
                .output();
        }
        for subvolume in &self.subvolumes {
            let _ = Command::new("btrfs")
                .args(["subvolume", "delete", subvolume])
                .output();
        }
        for directory in &self.directories {
            let _ = std::fs::remove_dir(directory);
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.run();
    }
}

fn enabled() -> bool {
    if std::env::var_os("SNAPALL_IT").is_none() {
        eprintln!("skipped: set SNAPALL_IT=1 (see dev/btrfs-fixture.sh)");
        return false;
    }
    true
}

fn backend() -> SnapperCli {
    SnapperCli::system().with_no_dbus(true)
}

#[test]
fn contract_holds_against_real_snapper() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let config = ConfigName::parse("snapall_it_a").unwrap();
    backend_contract(&backend(), &config);
}

#[test]
fn snapshot_all_covers_every_fixture_config() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let backend = backend();
    let point = Point::parse("itpoint-snapshot-all").unwrap();
    let report = snapshot_all(
        &backend,
        None,
        &Description::parse("it: snapshot_all").unwrap(),
        Some(&point),
    )
    .expect("plan and run");
    // Other configs may exist on the machine; assert on the fixture's own.
    for name in ["snapall_it_a", "snapall_it_b"] {
        let outcome = report
            .outcomes
            .iter()
            .find(|o| o.config.as_str() == name)
            .unwrap_or_else(|| panic!("{name} missing from {report:?}"));
        assert!(outcome.result.is_ok(), "{name}: {:?}", outcome.result);
        // the tag went in with the snapshot and comes back out of real snapper's listing
        let id = outcome.result.as_ref().copied().unwrap();
        let listed = backend.list_snapshots(&outcome.config).unwrap();
        let snapshot = listed
            .iter()
            .find(|s| s.id == id)
            .expect("the new snapshot is listed");
        assert_eq!(
            snapshot.point.as_ref(),
            Some(&point),
            "{name}: the restore point round-trips"
        );
    }
    // a snapshot taken without a point carries none
    let plain = backend
        .create_snapshot(
            &ConfigName::parse("snapall_it_a").unwrap(),
            &Description::parse("it: untagged").unwrap(),
        )
        .unwrap();
    let listed = backend
        .list_snapshots(&ConfigName::parse("snapall_it_a").unwrap())
        .unwrap();
    assert_eq!(listed.iter().find(|s| s.id == plain).unwrap().point, None);
}

/// The whole restore round trip on real Btrfs with real snapper: what the preview says, what the
/// restore does, that it is limited to one path when asked, that it is itself undoable, and what
/// it refuses. Real files, in the config's real subvolume.
#[test]
fn restoring_works_end_to_end_on_a_real_filesystem() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let backend = backend();
    let config = ConfigName::parse("snapall_it_b").unwrap();
    let base = backend.config_subvolume(&config).unwrap();
    let file = |name: &str| std::path::PathBuf::from(&base).join(format!("restore-it-{name}"));
    let path = |name: &str| file(name).to_string_lossy().into_owned();
    let read = |name: &str| std::fs::read_to_string(file(name)).ok();

    // History: keep and gone exist; a snapshot is taken; then they are changed, and one is added.
    std::fs::write(file("keep"), "version one").unwrap();
    std::fs::write(file("gone"), "to be deleted").unwrap();
    std::fs::write(file("other"), "other one").unwrap();
    let before = backend
        .create_snapshot(&config, &Description::parse("it: before changes").unwrap())
        .unwrap();
    std::fs::write(file("keep"), "version TWO").unwrap();
    std::fs::remove_file(file("gone")).unwrap();
    std::fs::write(file("added"), "new").unwrap();
    std::fs::write(file("other"), "other TWO").unwrap();

    // The preview names exactly those four, with the right kinds, and changes nothing.
    let preview = preview_restore(&backend, &config, before, None).unwrap();
    let mine: Vec<_> = preview
        .changes
        .iter()
        .filter(|c| c.path.contains("restore-it-"))
        .map(|c| {
            (
                c.path.rsplit("restore-it-").next().unwrap().to_owned(),
                c.kind,
            )
        })
        .collect();
    assert_eq!(
        mine,
        [
            ("added".to_owned(), ChangeKind::Added),
            ("gone".to_owned(), ChangeKind::Removed),
            ("keep".to_owned(), ChangeKind::Changed),
            ("other".to_owned(), ChangeKind::Changed),
        ]
    );
    assert!(!preview.is_system_root);
    assert_eq!(read("keep").as_deref(), Some("version TWO"));

    // Limited to one path: only that file moves.
    restore_snapshot(&backend, &config, before, Some(&path("keep"))).unwrap();
    assert_eq!(read("keep").as_deref(), Some("version one"));
    assert_eq!(read("other").as_deref(), Some("other TWO"), "not asked for");
    assert_eq!(read("gone"), None, "not asked for");

    // Everything: files come back, files created since go, edits revert; a safety snapshot first.
    let restored = restore_snapshot(&backend, &config, before, None).unwrap();
    assert_eq!(read("gone").as_deref(), Some("to be deleted"));
    assert_eq!(read("added"), None);
    assert_eq!(read("other").as_deref(), Some("other one"));
    assert!(restored.summary.total() >= 3, "{:?}", restored.summary);
    let listed = backend.list_snapshots(&config).unwrap();
    let note = listed
        .iter()
        .find(|s| s.id == restored.safety_snapshot)
        .unwrap();
    assert_eq!(note.description, format!("before restoring #{before}"));

    // ...and the restore itself can be undone through that snapshot.
    restore_snapshot(&backend, &config, restored.safety_snapshot, None).unwrap();
    assert_eq!(
        read("keep").as_deref(),
        Some("version one"),
        "keep was restored before the safety snapshot"
    );
    assert_eq!(read("other").as_deref(), Some("other TWO"));
    assert_eq!(read("added").as_deref(), Some("new"));
    assert_eq!(read("gone"), None);

    // Refusals happen before anything is touched.
    assert!(matches!(
        restore_snapshot(&backend, &config, before, Some("/etc/hostname")),
        Err(RestoreError::OutsideConfig { .. })
    ));
    let nowhere = snapall_core::SnapshotId::new(999_999).unwrap();
    assert!(matches!(
        restore_snapshot(&backend, &config, nowhere, None),
        Err(RestoreError::UnknownSnapshot { .. })
    ));

    for name in ["keep", "gone", "added", "other"] {
        let _ = std::fs::remove_file(file(name));
    }
}

/// A file's history on real Btrfs and real snapper: this is the test of the assumption the adapter
/// rests on, that snapper keeps snapshot N of a subvolume at `<subvolume>/.snapshots/N/snapshot`.
#[test]
fn a_files_history_is_read_from_real_snapshots_and_a_version_can_be_restored() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let backend = backend();
    let config = ConfigName::parse("snapall_it_a").unwrap();
    let base = backend.config_subvolume(&config).unwrap();
    let file = format!("{base}/history-it.txt");

    std::fs::write(&file, "first").unwrap();
    let one = backend
        .create_snapshot(&config, &Description::parse("it: history one").unwrap())
        .unwrap();
    std::fs::write(&file, "second, a little longer").unwrap();
    let two = backend
        .create_snapshot(&config, &Description::parse("it: history two").unwrap())
        .unwrap();

    let history = file_history(&backend, &file).unwrap();
    assert_eq!(
        history.config, config,
        "the config that covers the file is found by its mount point"
    );
    let state = |id| {
        history
            .versions
            .iter()
            .find(|v| v.snapshot.id == id)
            .map(|v| v.state)
    };
    assert_eq!(
        state(one),
        Some(FileState::Changed),
        "#1 had the old content"
    );
    assert_eq!(
        state(two),
        Some(FileState::Same),
        "#2 is the file as it is now"
    );

    // Restoring the old version through the same path the TUI uses, then the history agrees.
    restore_snapshot(&backend, &config, one, Some(&file)).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "first");
    let after = file_history(&backend, &file).unwrap();
    let again = |id| {
        after
            .versions
            .iter()
            .find(|v| v.snapshot.id == id)
            .map(|v| v.state)
    };
    assert_eq!(again(one), Some(FileState::Same));
    assert_eq!(again(two), Some(FileState::Changed));

    // A deleted file still has its history, and a directory is refused rather than guessed at.
    std::fs::remove_file(&file).unwrap();
    let gone = file_history(&backend, &file).unwrap();
    assert!(gone
        .versions
        .iter()
        .any(|v| v.snapshot.id == one && v.state == FileState::DeletedSince));
    assert!(file_history(&backend, &base).is_err());
}

/// The bug a user hit: `/home` is a separate subvolume beside the `/` config, so a snapshot of `/`
/// holds none of it, and the history of a file there said "did not exist" in every snapshot. On real
/// Btrfs, a separate subvolume inside a config's subvolume must be told apart from the config's own.
#[test]
fn a_separate_subvolume_inside_a_configs_directory_is_not_covered_by_that_config() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let backend = backend();
    let config = ConfigName::parse("snapall_it_b").unwrap();
    let base = backend.config_subvolume(&config).unwrap();
    let nested = format!("{base}/separate-it");
    let _cleanup = Cleanup {
        config: None,
        subvolumes: vec![nested.clone()],
        directories: Vec::new(),
    };
    _cleanup.run(); // what an earlier failed run left behind
    let run = |args: &[&str]| {
        let status = std::process::Command::new("btrfs")
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "btrfs {args:?}");
    };
    run(&["subvolume", "create", &nested]);
    let inside = format!("{base}/beside-it.txt");
    let apart = format!("{nested}/notes.txt");
    std::fs::write(&inside, "in the config's own subvolume").unwrap();
    std::fs::write(&apart, "in the separate one").unwrap();
    backend
        .create_snapshot(
            &config,
            &Description::parse("it: with a separate subvolume").unwrap(),
        )
        .unwrap();

    // the snapshot really does leave the separate subvolume out, which is the premise
    assert!(std::fs::read_dir(format!("{base}/.snapshots")).is_ok());
    assert!(backend.covers(&config, &inside).unwrap());
    assert!(
        !backend.covers(&config, &apart).unwrap(),
        "a nested subvolume is another device"
    );

    assert_eq!(config_covering(&backend, &inside).unwrap(), config);
    let error = config_covering(&backend, &apart).unwrap_err();
    assert!(
        matches!(error, HistoryError::OtherSubvolume { .. }),
        "{error}"
    );
    let history = file_history(&backend, &apart).unwrap_err();
    assert!(
        matches!(history, HistoryError::OtherSubvolume { .. }),
        "never \"did not exist\": {history}"
    );
    assert!(file_history(&backend, &inside).is_ok());

    run(&["subvolume", "delete", &nested]);
    let _ = std::fs::remove_file(&inside);
}

/// Setting an area up on real Btrfs with real snapper: what it creates, that snapshots on a schedule
/// end up off, that the new area is found by file lookups, and that a directory that is not a
/// subvolume fails cleanly and leaves nothing registered.
#[test]
fn an_area_can_be_set_up_on_a_real_subvolume_and_not_on_a_plain_directory() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let backend = backend();
    let base = backend
        .config_subvolume(&ConfigName::parse("snapall_it_b").unwrap())
        .unwrap();
    let area = format!("{base}/area-it");
    let plain = format!("{base}/plain-it");
    let _cleanup = Cleanup {
        config: Some("area-it".into()),
        subvolumes: vec![area.clone()],
        directories: vec![plain.clone()],
    };
    _cleanup.run(); // what an earlier failed run left behind
    let btrfs = |args: &[&str]| {
        assert!(
            std::process::Command::new("btrfs")
                .args(args)
                .status()
                .unwrap()
                .success(),
            "btrfs {args:?}"
        );
    };
    btrfs(&["subvolume", "create", &area]);
    std::fs::create_dir(&plain).unwrap();
    let before = backend.list_configs().unwrap();

    let name = protect_area(&backend, &area).unwrap();
    assert_eq!(name.as_str(), "area-it", "named after the directory");
    assert!(backend.list_configs().unwrap().contains(&name));
    assert!(
        std::path::Path::new(&format!("{area}/.snapshots")).exists(),
        "where the snapshots will be kept"
    );
    let settings = String::from_utf8(
        std::process::Command::new("snapper")
            .args(["--no-dbus", "-c", name.as_str(), "get-config"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let timeline = settings
        .lines()
        .find(|l| l.contains("TIMELINE_CREATE"))
        .unwrap();
    assert!(
        timeline.contains("no"),
        "snapshots on a schedule must be off: {timeline}"
    );

    // it takes part in a file lookup and in a snapshot straight away
    let file = format!("{area}/notes.txt");
    std::fs::write(&file, "mine").unwrap();
    assert_eq!(config_covering(&backend, &file).unwrap(), name);
    let id = backend
        .create_snapshot(
            &name,
            &Description::parse("it: first snapshot of a new area").unwrap(),
        )
        .unwrap();
    assert!(backend
        .list_snapshots(&name)
        .unwrap()
        .iter()
        .any(|s| s.id == id));

    // a second attempt is refused, and a plain directory fails with snapper's reason and leaves nothing
    assert!(matches!(
        protect_area(&backend, &area),
        Err(SetupError::AlreadyProtected { .. })
    ));
    let error = protect_area(&backend, &plain).unwrap_err();
    assert!(matches!(error, SetupError::Failed(_)), "{error}");
    assert!(
        error.to_string().contains("not a btrfs subvolume"),
        "{error}"
    );
    assert_eq!(
        backend.list_configs().unwrap().len(),
        before.len() + 1,
        "the failed one registered nothing"
    );

    // undo, as the rollback does (this also removes the snapshots it held: nothing else does)
    backend.delete_config(&name).unwrap();
    assert_eq!(backend.list_configs().unwrap(), before);
    btrfs(&["subvolume", "delete", &area]);
    std::fs::remove_dir(&plain).unwrap();
}

/// The whole restore point round trip on real Btrfs with real snapper: one action snapshots two
/// areas, both are ruined, one action puts both back, and one action undoes that. Real files in two
/// real subvolumes, the tag read back from snapper, the safety snapshots found as a point of their own.
#[test]
fn a_restore_point_puts_two_real_areas_back_with_one_action_and_that_is_undoable() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let backend = backend();
    let a = ConfigName::parse("snapall_it_a").unwrap();
    let b = ConfigName::parse("snapall_it_b").unwrap();
    let (dir_a, dir_b) = (
        backend.config_subvolume(&a).unwrap(),
        backend.config_subvolume(&b).unwrap(),
    );
    let (file_a, file_b) = (
        format!("{dir_a}/point-it.txt"),
        format!("{dir_b}/point-it.txt"),
    );
    std::fs::write(&file_a, "a, then").unwrap();
    std::fs::write(&file_b, "b, then").unwrap();

    let point = fresh_point(&backend, 1_700_000_000);
    snapshot_all(
        &backend,
        Some(&[a.clone(), b.clone()]),
        &Description::parse("it: a point").unwrap(),
        Some(&point),
    )
    .unwrap();
    std::fs::write(&file_a, "a, ruined").unwrap();
    std::fs::remove_file(&file_b).unwrap();

    let found = restore_points(&all_snapshots(&backend).unwrap());
    let ours = found
        .iter()
        .find(|p| p.point == point)
        .expect("the point is found by its tag, in real snapper's listing");
    assert!(ours.tagged);
    let areas: Vec<&str> = ours.members.iter().map(|m| m.config.as_str()).collect();
    assert!(
        areas.contains(&"snapall_it_a") && areas.contains(&"snapall_it_b"),
        "{areas:?}"
    );

    let safety = fresh_point(&backend, 1_700_000_001);
    let done = restore_point(&backend, ours, safety.clone());
    assert!(done.succeeded(), "{done:?}");
    assert_eq!(std::fs::read_to_string(&file_a).unwrap(), "a, then");
    assert_eq!(
        std::fs::read_to_string(&file_b).unwrap(),
        "b, then",
        "a deleted file is brought back"
    );

    // the way back is a restore point too, found the same way
    let found = restore_points(&all_snapshots(&backend).unwrap());
    let undo = found
        .iter()
        .find(|p| p.point == safety)
        .expect("the safety snapshots are one point");
    assert!(
        undo.description.starts_with("before restoring "),
        "{}",
        undo.description
    );
    let again = restore_point(&backend, undo, fresh_point(&backend, 1_700_000_002));
    assert!(
        again.succeeded()
            && again
                .members
                .iter()
                .any(|m| matches!(m.result, MemberResult::Restored(_))),
        "{again:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&file_a).unwrap(),
        "a, ruined",
        "back to just before"
    );
    assert!(!std::path::Path::new(&file_b).exists());

    let _ = std::fs::remove_file(&file_a);
}

/// Real snapper reports what the health screen is built on: an area's retention settings, and, only when
/// Btrfs quotas are on, the bytes each snapshot alone holds (a deleted 3 MB file shows up as 3 MB).
#[test]
fn real_snapper_reports_retention_and_measures_a_snapshots_room_only_with_quotas() {
    if !enabled() {
        return;
    }
    let _turn = take_turns();
    let backend = backend();
    let a = ConfigName::parse("snapall_it_a").unwrap();
    let dir = backend.config_subvolume(&a).unwrap();
    let mount = std::path::Path::new(&dir)
        .parent()
        .unwrap()
        .display()
        .to_string();

    let retention = backend.retention(&a).unwrap();
    assert_eq!(retention.keep, Some(50), "snapper's default for a new area");
    assert!(
        retention.cleanup && retention.min_age_seconds == Some(1800),
        "{retention:?}"
    );

    let btrfs = |args: &[&str]| {
        let _ = std::process::Command::new("btrfs").args(args).output();
    };
    btrfs(&["quota", "disable", &mount]);
    let file = format!("{dir}/room-it.bin");
    std::fs::write(&file, vec![7u8; 3_000_000]).unwrap();
    let before = backend
        .create_snapshot(&a, &Description::parse("it: room, no quotas").unwrap())
        .unwrap();
    let unmeasured = backend.list_snapshots(&a).unwrap();
    assert_eq!(
        unmeasured
            .iter()
            .find(|s| s.id == before)
            .unwrap()
            .used_bytes,
        None,
        "not measured is not zero"
    );

    // The file is in that snapshot too, and room two snapshots share is held by neither alone (which is
    // what "exclusive" means), so it goes before the one that is measured.
    backend.delete_snapshot(&a, before).unwrap();
    // deleting is lazy: wait until the cleaner has really let go of it
    btrfs(&["subvolume", "sync", &mount]);
    btrfs(&["quota", "enable", &mount]);
    btrfs(&["quota", "rescan", "-w", &mount]);
    let id = backend
        .create_snapshot(&a, &Description::parse("it: room, quotas on").unwrap())
        .unwrap();
    std::fs::remove_file(&file).unwrap();
    btrfs(&["filesystem", "sync", &mount]);
    btrfs(&["quota", "rescan", "-w", &mount]);
    let measured = backend.list_snapshots(&a).unwrap();
    let held = measured.iter().find(|s| s.id == id).unwrap().used_bytes;
    btrfs(&["quota", "disable", &mount]);
    assert!(
        held.is_some_and(|bytes| bytes >= 2_900_000),
        "the deleted 3 MB file is held by the snapshot alone: {held:?}"
    );
}
