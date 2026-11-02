// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contract every backend must satisfy, as one function to run against it.

use crate::ports::snapshots::SnapshotBackend;
use crate::vocabulary::domain::{AreaName, Description, FileState, SnapshotId, SnapshotKind};

/// The behaviour every `SnapshotBackend` must have, whatever sits behind it.
/// `existing` must name an area the backend can snapshot.
pub fn backend_contract(backend: &dyn SnapshotBackend, existing: &AreaName) {
    let missing = AreaName::parse("snapall-no-such-area").expect("valid name");
    the_area_is_listed(backend, existing);
    let (first, second) = creating(backend, existing, &missing);
    listing_snapshots(backend, existing, &missing, second);
    let location = reading_what_changed(backend, existing, &missing, second);
    reading_a_files_history(backend, existing, &missing, &location);
    deleting(backend, existing, first, second);
}

fn the_area_is_listed(backend: &dyn SnapshotBackend, existing: &AreaName) {
    let areas = backend.list_areas().expect("list_areas succeeds");
    assert!(
        areas.contains(existing),
        "list_areas should include `{existing}`, got {areas:?}"
    );
}

/// Two snapshots with increasing numbers; a missing area is an error. Returns the two numbers.
fn creating(
    backend: &dyn SnapshotBackend,
    existing: &AreaName,
    missing: &AreaName,
) -> (SnapshotId, SnapshotId) {
    let description = Description::parse("snapall contract").expect("valid description");
    let first = backend
        .create_snapshot(existing, &description)
        .expect("first snapshot succeeds");
    let second = backend
        .create_snapshot(existing, &description)
        .expect("second snapshot succeeds");
    assert!(second > first, "ids must increase: {first} then {second}");
    assert!(
        backend.create_snapshot(missing, &description).is_err(),
        "a snapshot of a missing area must be an error"
    );
    (first, second)
}

/// Oldest first, and what was created is there, as it was described.
fn listing_snapshots(
    backend: &dyn SnapshotBackend,
    existing: &AreaName,
    missing: &AreaName,
    second: SnapshotId,
) {
    let listed = backend
        .list_snapshots(existing)
        .expect("list_snapshots succeeds");
    assert!(
        listed.windows(2).all(|pair| pair[0].id < pair[1].id),
        "snapshots must be listed oldest first: {listed:?}"
    );
    let created = listed
        .iter()
        .find(|s| s.id == second)
        .unwrap_or_else(|| panic!("snapshot {second} should be listed: {listed:?}"));
    assert_eq!(created.description, "snapall contract");
    assert_eq!(created.kind, SnapshotKind::Single);
    assert!(
        backend.list_snapshots(missing).is_err(),
        "listing a missing area must be an error"
    );
}

/// Restoring is checked read-only here (mutation is backend-specific): where the area lives, what changed since a
/// snapshot, and that a snapshot that does not exist is an error. Returns where the area lives.
fn reading_what_changed(
    backend: &dyn SnapshotBackend,
    existing: &AreaName,
    missing: &AreaName,
    second: SnapshotId,
) -> String {
    let location = backend
        .area_location(existing)
        .expect("area_location succeeds");
    assert!(
        location.starts_with('/'),
        "a mount point is absolute: {location}"
    );
    backend
        .changes_since(existing, second)
        .expect("changes_since an existing snapshot succeeds");
    let nowhere = SnapshotId::new(999_999).expect("nonzero");
    assert!(
        backend.changes_since(existing, nowhere).is_err(),
        "changes since a snapshot that does not exist must be an error"
    );
    assert!(
        backend.undo_changes(existing, nowhere, None).is_err(),
        "undoing to a snapshot that does not exist must be an error"
    );
    assert!(backend.area_location(missing).is_err());
    assert!(
        backend
            .covers(existing, &location)
            .expect("covers succeeds"),
        "an area covers its own location"
    );
    location
}

/// A file's history is read-only too: for a path that was never there, every snapshot says so.
fn reading_a_files_history(
    backend: &dyn SnapshotBackend,
    existing: &AreaName,
    missing: &AreaName,
    location: &str,
) {
    let never = format!(
        "{}/snapall-contract-never-existed",
        location.trim_end_matches('/')
    );
    let history = backend
        .file_versions(existing, &never)
        .expect("file_versions succeeds for a path that does not exist");
    assert!(
        history.iter().all(|v| v.state == FileState::AbsentThen),
        "a file that never existed is absent in every snapshot"
    );
    assert!(backend.file_versions(missing, &never).is_err());
}

/// Gone afterwards, the others survive, and deleting again is an error, not a silent success.
fn deleting(
    backend: &dyn SnapshotBackend,
    existing: &AreaName,
    first: SnapshotId,
    second: SnapshotId,
) {
    backend
        .delete_snapshot(existing, first)
        .expect("deleting an existing snapshot succeeds");
    let after = backend
        .list_snapshots(existing)
        .expect("list_snapshots succeeds");
    assert!(
        !after.iter().any(|s| s.id == first),
        "snapshot {first} should be gone: {after:?}"
    );
    assert!(
        after.iter().any(|s| s.id == second),
        "other snapshots must survive a delete"
    );
    assert!(
        backend.delete_snapshot(existing, first).is_err(),
        "deleting a snapshot twice must be an error"
    );
}
