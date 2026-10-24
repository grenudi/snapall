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
    preview_restore, restore_snapshot, snapshot_all, ChangeKind, ConfigName, Description,
    RestoreError, SnapshotBackend,
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
