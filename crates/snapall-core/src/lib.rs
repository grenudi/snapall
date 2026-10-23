//! Domain, planning and ports for snapall.
//!
//! This crate is the hexagon: it knows *what* snapall does and nothing about *how* a
//! snapshot is taken. It has no I/O and must not depend on a CLI parser, a serializer
//! or an adapter; `crates/snapall/tests/architecture.rs` enforces that.

pub mod domain;
pub mod plan;
pub mod port;
pub mod usecase;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use domain::{ConfigName, Description, ParseError, SnapshotId};
pub use plan::{plan, Plan, PlanError};
pub use port::{BackendError, SnapshotBackend};
pub use usecase::{plan_snapshots, run_plan, snapshot_all, Outcome, Report, SnapshotError};
