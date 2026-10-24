// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Domain, planning and ports for snapall.
//!
//! This crate is the hexagon: it knows *what* snapall does and nothing about *how* a
//! snapshot is taken. It has no I/O and must not depend on a CLI parser, a serializer
//! or an adapter; `crates/snapall/tests/architecture.rs` enforces that.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod boot;
pub mod doctor;
pub mod domain;
pub mod observe;
pub mod plan;
pub mod port;
pub mod usecase;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use domain::{
    Change, ChangeKind, ConfigName, Description, FileState, FileVersion, ParseError,
    RestoreSummary, SnapshotId, SnapshotInfo, SnapshotKind,
};
pub use observe::{Event, Observer, Silent};
pub use plan::{plan, Plan, PlanError};
pub use port::{BackendError, SnapshotBackend};
pub use usecase::{
    all_snapshots, config_covering, delete_snapshot, file_history, file_versions_of,
    plan_snapshots, preview_restore, restore_snapshot, run_plan, snapshot_all, snapshots_of,
    FileHistory, HistoryError, Outcome, Preview, Report, RestoreError, Restored, SnapshotError,
};
