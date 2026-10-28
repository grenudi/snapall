// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What looking brought back, as the window holds it: areas, a file's versions, a restore point's offer, what a restore did.

use crate::model::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load<T> {
    NotLoaded,
    Loaded(T),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaEntry {
    pub name: AreaName,
    /// Oldest first, as the port returns them. The view shows newest first.
    pub snapshots: Load<Vec<SnapshotInfo>>,
}

/// One file the UI was opened on: where it is, and the area whose snapshots hold its history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTarget {
    pub path: String,
    pub area: AreaName,
}

/// The file history screen's own state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileView {
    pub target: FileTarget,
    /// Oldest first, as the backend gives them; shown newest first.
    pub versions: Load<Vec<FileVersion>>,
    pub cursor: usize,
}

/// The restore point a snapshot belongs to, and what putting all of it back would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointOffer {
    pub point: snapall_core::RestorePoint,
    pub views: Vec<snapall_core::MemberPreview>,
}

impl PointOffer {
    /// How many areas would change.
    pub fn areas_to_change(&self) -> usize {
        self.views
            .iter()
            .filter(|v| {
                matches!(&v.view, snapall_core::MemberView::Changes(p) if !p.changes.is_empty())
            })
            .count()
    }
}

/// What putting a restore point back did, boiled down to what a person is told.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PointOutcome {
    pub restored: usize,
    pub unchanged: usize,
    pub failed: Vec<String>,
    /// The restore point of the safety snapshots: restoring it undoes everything, if anything changed.
    pub undo: Option<String>,
}
