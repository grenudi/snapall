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
pub mod calendar;
pub mod consequence;
pub mod course;
pub mod coverage;
pub mod doctor;
pub mod domain;
pub mod explain;
pub mod groups;
pub mod health;
pub mod journal;
pub mod limits;
pub mod observe;
pub mod plan;
pub mod points;
pub mod port;
pub mod rollback;
pub mod services;
pub mod system;
pub mod unprotect;
pub mod usecase;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use consequence::{Act, Weight};
pub use course::{Course, Unattended};
pub use domain::{
    Change, ChangeKind, ConfigName, ConfigOption, Description, FileState, FileVersion, ParseError,
    Point, RestoreSummary, Retention, SnapshotId, SnapshotInfo, SnapshotKind,
};
pub use groups::{GroupName, Groups, GroupsError, Wanted};
pub use health::{
    assess, gather, AreaFacts, Chip, Facts, Finding, Health, Level, Remedy, Topic, Verdict,
};
pub use journal::{
    Entry as JournalEntry, Event as JournalEvent, Outcome as JournalOutcome,
    Progress as JournalProgress, Rollback, Source, Status as JournalStatus, Undo,
};
pub use limits::{
    preview as preview_limits, preview_words as limits_words, Limits, Preview as LimitsPreview,
};
pub use observe::{Event, Observer, Silent};
pub use plan::{plan, Plan, PlanError};
pub use points::{
    fresh_point, point_of, preview_point, restore_point, restore_point_along, restore_points,
    Member, MemberPreview, MemberReport, MemberResult, MemberView, PointRestore, RestorePoint,
};
pub use port::{BackendError, SnapshotBackend};
pub use services::{Command, Job, JobAction};
pub use system::{
    Busy, DiskUsage, Hold, NoSystem, QuotaMode, ServiceState, SystemProbe, UnitDetails,
};
pub use usecase::{
    all_snapshots, config_covering, delete_snapshot, file_history, file_versions_of,
    plan_snapshots, preview_restore, protect_area, restore_snapshot, restore_snapshot_with,
    run_plan, run_plan_along, snapshot_all, snapshots_of, FileHistory, HistoryError, Outcome,
    Preview, Report, RestoreError, Restored, Safety, SetupError, SnapshotError,
};
