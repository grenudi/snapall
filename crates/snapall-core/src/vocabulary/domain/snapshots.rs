// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What is known about a snapshot.

use super::*;

/// How a snapshot relates to others. A `Post` always knows its `Pre`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind {
    Single,
    Pre,
    Post { pre: SnapshotId },
}

/// One existing snapshot, as the backend reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub id: SnapshotId,
    pub kind: SnapshotKind,
    /// When it was taken, in the backend's own format (for snapper `2026-10-23 01:26:30`).
    pub date: Option<String>,
    pub user: String,
    /// The cleanup algorithm that may prune it (`number`, `timeline`, ...), if any.
    pub cleanup: Option<String>,
    /// Free text stored with the snapshot. Not a `Description`: it may have been written by
    /// anything, so it is shown, never trusted.
    pub description: String,
    /// The restore point it belongs to, if one action made it together with snapshots of other areas.
    pub point: Option<Point>,
    /// The bytes only this snapshot holds, when Btrfs quotas make that knowable; `None` otherwise.
    pub used_bytes: Option<u64>,
}
