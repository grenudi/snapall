// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated domain types. Untrusted text is parsed into these once, at the boundary;
//! everything downstream receives a value that cannot be invalid.

mod files;
mod keeping;
mod names;
mod snapshots;
#[cfg(test)]
mod tests;

pub use files::{Change, ChangeKind, FileState, FileVersion, RestoreSummary};
pub use keeping::{AreaOption, Retention};
pub use names::{AreaName, Description, ParseError, Point, SnapshotId};
pub use snapshots::{SnapshotInfo, SnapshotKind};
