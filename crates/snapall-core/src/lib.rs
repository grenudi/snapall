// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Domain, planning and ports for snapall.
//!
//! This crate is the hexagon: it knows *what* snapall does and nothing about *how* a
//! snapshot is taken. It has no I/O and must not depend on a CLI parser, a serializer
//! or an adapter; `crates/snapall/tests/architecture.rs` enforces that.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod going_back;
pub mod health;
pub mod keeping;
pub mod looking;
pub mod ports;
pub mod protecting;
pub mod taking;
pub mod vocabulary;

#[cfg(test)]
mod journeys;

pub use going_back::points::{
    fresh_point, point_of, preview_point, restore_point, restore_point_along, restore_points,
    Member, MemberPreview, MemberReport, MemberResult, MemberView, PointRestore, RestorePoint,
};
pub use going_back::restoring::{
    preview_restore, restore_snapshot, restore_snapshot_with, Preview, RestoreError, Restored,
    Safety,
};
pub use health::{
    assess, gather, AreaFacts, Chip, Facts, Finding, Health, Level, Remedy, Topic, Verdict,
};
pub use keeping::groups::{GroupName, Groups, GroupsError, Wanted};
pub use keeping::jobs::{Command, Job, JobAction};
pub use keeping::limits::{
    preview as preview_limits, preview_words as limits_words, Limits, Preview as LimitsPreview,
};
pub use looking::find::find_snapshot;
pub use looking::history::{
    area_covering, file_history, file_versions_of, FileHistory, HistoryError,
};
pub use looking::journal::{
    Entry as JournalEntry, Event as JournalEvent, Outcome as JournalOutcome,
    Progress as JournalProgress, Rollback, Source, Status as JournalStatus, Undo,
};
pub use looking::snapshots::{all_snapshots, delete_snapshot, snapshots_of, SnapshotError};
pub use ports::course::{Course, Unattended};
pub use ports::machine::{
    Busy, DiskUsage, Hold, NoSystem, QuotaMode, ServiceState, Switching, SystemProbe, UnitDetails,
};
pub use ports::observing::{Event, Observer, Silent};
pub use ports::snapshots::{BackendError, SnapshotBackend};
pub use protecting::protect::{protect_area, SetupError};
pub use taking::plan::{plan, Plan, PlanError};
pub use taking::run::{plan_snapshots, run_plan, run_plan_along, snapshot_all, Outcome, Report};
pub use vocabulary::consequence::{Act, Weight};
pub use vocabulary::domain::{
    AreaName, AreaOption, Change, ChangeKind, Description, FileState, FileVersion, ParseError,
    Point, RestoreSummary, Retention, SnapshotId, SnapshotInfo, SnapshotKind,
};
