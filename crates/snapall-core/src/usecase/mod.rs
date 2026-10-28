// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application use case, written against the port only.

mod history;
mod protecting;
mod restoring;
mod snapshots;
mod taking;
#[cfg(test)]
mod tests;

pub use history::{area_covering, file_history, file_versions_of, FileHistory, HistoryError};
pub use protecting::{protect_area, SetupError};
pub(crate) use restoring::is_root;
use restoring::{clean, under};
pub use restoring::{
    preview_restore, restore_snapshot, restore_snapshot_with, Preview, RestoreError, Restored,
    Safety,
};
use snapshots::ensure_area;
pub use snapshots::{all_snapshots, delete_snapshot, snapshots_of, SnapshotError};
pub use taking::{plan_snapshots, run_plan, run_plan_along, snapshot_all, Outcome, Report};
