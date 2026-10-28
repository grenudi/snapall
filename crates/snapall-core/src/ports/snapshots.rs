// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall needs from whatever takes snapshots, said in snapall's own words.

use thiserror::Error;

use crate::vocabulary::domain::{
    AreaName, AreaOption, Change, Description, FileVersion, Point, RestoreSummary, Retention,
    SnapshotId, SnapshotInfo,
};

pub trait SnapshotBackend {
    /// Every area the backend manages.
    fn list_areas(&self) -> Result<Vec<AreaName>, BackendError>;

    /// Take one snapshot of `config` and return its number.
    /// Like [`SnapshotBackend::create_snapshot`], and the snapshot is marked as part of `point`, so the
    /// snapshots of every area made by one action can be found together again.
    fn create_snapshot_in(
        &self,
        area: &AreaName,
        description: &Description,
        point: &Point,
    ) -> Result<SnapshotId, BackendError>;

    fn create_snapshot(
        &self,
        area: &AreaName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError>;

    /// The snapshots of `config`, oldest first (ascending id). The "current system"
    /// pseudo-snapshot is not a snapshot and is never included.
    fn list_snapshots(&self, area: &AreaName) -> Result<Vec<SnapshotInfo>, BackendError>;

    /// Delete one snapshot of `config`. Deleting one that does not exist is an error.
    fn delete_snapshot(&self, area: &AreaName, id: SnapshotId) -> Result<(), BackendError>;

    /// Where the area's subvolume is mounted. `/` means the system root.
    fn area_location(&self, area: &AreaName) -> Result<String, BackendError>;

    /// What differs between snapshot `id` and the live system, by absolute path. Read-only.
    fn changes_since(&self, area: &AreaName, id: SnapshotId) -> Result<Vec<Change>, BackendError>;

    /// Start keeping snapshots of the subvolume at `subvolume` (an absolute path), under `name`.
    /// Nothing is copied or moved: it creates the place the snapshots will be kept.
    fn create_area(&self, name: &AreaName, location: &str) -> Result<(), BackendError>;

    /// Stop keeping snapshots for `name` and remove its place. For undoing a half-finished
    /// [`SnapshotBackend::create_area`].
    fn delete_area(&self, name: &AreaName) -> Result<(), BackendError>;

    /// Choose one of an area's settings.
    fn set_area_option(&self, name: &AreaName, option: AreaOption) -> Result<(), BackendError>;

    /// How the area's snapshots are kept and cleaned up.
    fn retention(&self, area: &AreaName) -> Result<Retention, BackendError>;

    /// Whether `path` is on the area's own subvolume, so that a snapshot of the area contains it.
    ///
    /// Being under the subvolume's directory is not enough: a snapshot stops at a subvolume
    /// boundary, so a file in a separate subvolume that is nested in or mounted over that directory
    /// (the usual `/home` beside a `/` area) is not in the area's snapshots at all.
    fn covers(&self, area: &AreaName, path: &str) -> Result<bool, BackendError>;

    /// How the file at `path` (absolute, inside the area's subvolume) stood in every snapshot of
    /// the area, oldest first, compared with the file as it is now. Read-only. A directory is
    /// [`BackendError::NotSupported`].
    fn file_versions(&self, area: &AreaName, path: &str) -> Result<Vec<FileVersion>, BackendError>;

    /// Make the live system match snapshot `id` again, for everything in the area or only
    /// for `path` (and what is under it). Files created since are removed, deleted ones come
    /// back, edited ones are reverted. It does not take a safety snapshot first: the use case
    /// ([`crate::going_back::restoring::restore_snapshot`]) does, because that is policy, not mechanism.
    fn undo_changes(
        &self,
        area: &AreaName,
        id: SnapshotId,
        path: Option<&str>,
    ) -> Result<RestoreSummary, BackendError>;
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("could not run `{program}`: {detail}")]
    Unavailable { program: String, detail: String },
    #[error("`{program}` failed ({status}): {stderr}")]
    CommandFailed {
        program: String,
        status: String,
        stderr: String,
    },
    #[error("unexpected output from `{program}`: {detail}")]
    UnexpectedOutput { program: String, detail: String },
    #[error("`{0}` is not a protected area (`snapall list` shows the ones that exist)")]
    UnknownArea(AreaName),
    /// The backend cannot do what was asked of it, and says why.
    #[error("{0}")]
    NotSupported(String),
}
