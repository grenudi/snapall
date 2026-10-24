// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The outbound port: everything snapall needs from "something that takes snapshots".

use thiserror::Error;

use crate::domain::{Change, ConfigName, Description, RestoreSummary, SnapshotId, SnapshotInfo};

pub trait SnapshotBackend {
    /// Every config the backend manages.
    fn list_configs(&self) -> Result<Vec<ConfigName>, BackendError>;

    /// Take one snapshot of `config` and return its number.
    fn create_snapshot(
        &self,
        config: &ConfigName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError>;

    /// The snapshots of `config`, oldest first (ascending id). The "current system"
    /// pseudo-snapshot is not a snapshot and is never included.
    fn list_snapshots(&self, config: &ConfigName) -> Result<Vec<SnapshotInfo>, BackendError>;

    /// Delete one snapshot of `config`. Deleting one that does not exist is an error.
    fn delete_snapshot(&self, config: &ConfigName, id: SnapshotId) -> Result<(), BackendError>;

    /// Where the config's subvolume is mounted. `/` means the system root.
    fn config_subvolume(&self, config: &ConfigName) -> Result<String, BackendError>;

    /// What differs between snapshot `id` and the live system, by absolute path. Read-only.
    fn changes_since(
        &self,
        config: &ConfigName,
        id: SnapshotId,
    ) -> Result<Vec<Change>, BackendError>;

    /// Make the live system match snapshot `id` again, for everything in the config or only
    /// for `path` (and what is under it). Files created since are removed, deleted ones come
    /// back, edited ones are reverted. It does not take a safety snapshot first: the use case
    /// ([`crate::usecase::restore_snapshot`]) does, because that is policy, not mechanism.
    fn undo_changes(
        &self,
        config: &ConfigName,
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
    #[error("config `{0}` does not exist")]
    UnknownConfig(ConfigName),
}
