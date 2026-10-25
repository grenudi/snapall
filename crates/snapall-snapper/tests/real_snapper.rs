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
    config_covering, file_history, preview_restore, restore_snapshot, snapshot_all, ChangeKind,
    ConfigName, Description, FileState, HistoryError, RestoreError, SnapshotBackend,
};
use snapall_snapper::SnapperCli;

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
    let config = ConfigName::parse("snapall_it_a").unwrap();
    backend_contract(&backend(), &config);
}

#[test]
fn snapshot_all_covers_every_fixture_config() {
    if !enabled() {
        return;
    }
    let report = snapshot_all(
        &backend(),
        None,
        &Description::parse("it: snapshot_all").unwrap(),
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
    }
}

/// The whole restore round trip on real Btrfs with real snapper: what the preview says, what the
/// restore does, that it is limited to one path when asked, that it is itself undoable, and what
/// it refuses. Real files, in the config's real subvolume.
#[test]
fn restoring_works_end_to_end_on_a_real_filesystem() {
    if !enabled() {
        return;
    }
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
    let backend = backend();
    let config = ConfigName::parse("snapall_it_b").unwrap();
    let base = backend.config_subvolume(&config).unwrap();
    let nested = format!("{base}/separate-it");
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
