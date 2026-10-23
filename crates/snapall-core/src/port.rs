//! The outbound port: everything snapall needs from "something that takes snapshots".

use thiserror::Error;

use crate::domain::{ConfigName, Description, SnapshotId};

pub trait SnapshotBackend {
    /// Every config the backend manages.
    fn list_configs(&self) -> Result<Vec<ConfigName>, BackendError>;

    /// Take one snapshot of `config` and return its number.
    fn create_snapshot(
        &self,
        config: &ConfigName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError>;
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
