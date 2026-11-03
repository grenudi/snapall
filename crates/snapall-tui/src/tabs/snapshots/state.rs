// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the Snapshots tab holds: the protected areas and the snapshots of the chosen one, the cursors, what is
//! marked, and which pane has the focus.

use crate::model::*;
use snapall_core::{AreaName, SnapshotId, SnapshotInfo};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotsTab {
    pub areas: Load<Vec<AreaEntry>>,
    pub area_cursor: usize,
    /// Index into the *displayed* (newest-first) snapshot list.
    pub snapshot_cursor: usize,
    pub marked: BTreeSet<SnapshotId>,
    pub focus: Pane,
}

impl Default for SnapshotsTab {
    fn default() -> Self {
        Self {
            areas: Load::NotLoaded,
            area_cursor: 0,
            snapshot_cursor: 0,
            marked: BTreeSet::new(),
            focus: Pane::Areas,
        }
    }
}

impl SnapshotsTab {
    pub fn entries(&self) -> &[AreaEntry] {
        match &self.areas {
            Load::Loaded(entries) => entries,
            _ => &[],
        }
    }

    pub fn selected_area(&self) -> Option<&AreaEntry> {
        self.entries().get(self.area_cursor)
    }

    /// The chosen area's name, for the questions and the reloads that need it after the borrow has ended.
    pub fn selected_area_name(&self) -> Option<AreaName> {
        self.selected_area().map(|entry| entry.name.clone())
    }

    /// The selected area's snapshots, newest first.
    pub fn displayed_snapshots(&self) -> Vec<&SnapshotInfo> {
        match self.selected_area().map(|c| &c.snapshots) {
            Some(Load::Loaded(list)) => list.iter().rev().collect(),
            _ => Vec::new(),
        }
    }

    pub fn selected_snapshot(&self) -> Option<&SnapshotInfo> {
        self.displayed_snapshots()
            .get(self.snapshot_cursor)
            .copied()
    }

    pub(crate) fn area_names(&self) -> Vec<AreaName> {
        self.entries().iter().map(|e| e.name.clone()).collect()
    }

    pub(crate) fn entry_mut(&mut self, name: &AreaName) -> Option<&mut AreaEntry> {
        match &mut self.areas {
            Load::Loaded(entries) => entries.iter_mut().find(|e| &e.name == name),
            _ => None,
        }
    }
}
