// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the in-memory stand-in remembers: each area, its snapshots and files.

use crate::vocabulary::domain::{AreaName, SnapshotInfo};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct AreaState {
    pub(super) next_id: u64,
    pub(super) snapshots: Vec<SnapshotInfo>,
    /// Where the config is mounted.
    pub(super) location: String,
    /// The live files: absolute path to content.
    pub(super) files: BTreeMap<String, String>,
    /// The files as each snapshot saw them, by snapshot number.
    pub(super) saved: BTreeMap<u64, BTreeMap<String, String>>,
    /// Whether snapshots are taken on a schedule, as snapper does for a new config.
    pub(super) timeline_create: bool,
    pub(super) keep: u32,
    pub(super) min_age: u64,
    pub(super) cleanup: bool,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) areas: BTreeMap<AreaName, AreaState>,
    pub(super) failing: BTreeSet<AreaName>,
    /// Directories that are the root of a separate subvolume: not part of any snapshot of a config
    /// above them, as a separate `/home` is not part of a snapshot of `/`.
    pub(super) separate: Vec<String>,
    /// Operations made to fail, to test what is undone when one does.
    pub(super) refuse_option_changes: bool,
    pub(super) refuse_removals: bool,
}

/// A backend that keeps everything in memory. Fast core tests need nothing else.
#[derive(Default)]
pub struct InMemoryBackend {
    pub(super) state: Mutex<State>,
}
