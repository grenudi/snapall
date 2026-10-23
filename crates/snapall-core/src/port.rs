//! The outbound port: everything snapall needs from "something that takes snapshots".

use thiserror::Error;

use crate::domain::{ConfigName, Description, SnapshotId, SnapshotInfo};

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
