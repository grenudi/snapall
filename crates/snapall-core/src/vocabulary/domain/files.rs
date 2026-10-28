// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What changed in a file between a snapshot and now, and what a restore did.

use super::*;

/// How one path differs between a snapshot and the live system now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// It exists now and did not then: restoring would remove it.
    Added,
    /// It existed then and is gone now: restoring would bring it back.
    Removed,
    /// It exists in both and differs (content, type, permissions, owner...): restoring would revert it.
    Changed,
}

/// One path that differs between a snapshot and the live system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    /// An absolute path inside the area's subvolume.
    pub path: String,
}

/// What a restore did, as snapper counts it: files recreated, reverted and removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestoreSummary {
    pub created: u64,
    pub modified: u64,
    pub deleted: u64,
}

impl RestoreSummary {
    pub fn total(&self) -> u64 {
        self.created + self.modified + self.deleted
    }
}

/// How a file as one snapshot had it compares with the file as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// The same content, type, permissions and owner: restoring would change nothing.
    Same,
    /// It exists in both and differs: restoring would revert it.
    Changed,
    /// It existed then and is gone now: restoring would bring it back.
    DeletedSince,
    /// It did not exist then: there is nothing in this snapshot to restore.
    AbsentThen,
}

impl FileState {
    /// What to tell a person: how the file in the snapshot compares with the file now.
    pub fn label(self) -> &'static str {
        match self {
            FileState::Same => "same as now",
            FileState::Changed => "differs from now",
            FileState::DeletedSince => "deleted since",
            FileState::AbsentThen => "did not exist",
        }
    }

    /// A stable token for machine-readable output.
    pub fn token(self) -> &'static str {
        match self {
            FileState::Same => "same",
            FileState::Changed => "changed",
            FileState::DeletedSince => "deleted_since",
            FileState::AbsentThen => "absent_then",
        }
    }

    /// Whether restoring from a snapshot in this state would put something back.
    pub fn restorable(self) -> bool {
        matches!(self, FileState::Changed | FileState::DeletedSince)
    }
}

/// One snapshot, and how the file in question stood in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileVersion {
    pub snapshot: SnapshotInfo,
    pub state: FileState,
}
