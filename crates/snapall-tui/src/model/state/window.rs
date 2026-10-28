// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window at one moment: its lists, cursor, open question, message, and what it is waiting for.

use super::*;
use crate::model::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Info(String),
    Error(String),
}

/// A change this window handed to the background: its name, and how many times the journal was read
/// without it appearing there (a change that never reports is eventually let go of).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    pub id: String,
    pub unseen: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub areas: Load<Vec<AreaEntry>>,
    pub area_cursor: usize,
    /// Index into the *displayed* (newest-first) snapshot list.
    pub snapshot_cursor: usize,
    pub marked: BTreeSet<SnapshotId>,
    pub focus: Pane,
    pub modal: Option<Modal>,
    pub notice: Option<Notice>,
    pub quit: bool,
    /// Effects handed to the shell whose results have not come back. While it is above zero the
    /// screen says "working" and the keys that would start more work are refused. A shell that
    /// runs effects instantly (the tests) never sees it above zero between messages.
    pub pending: usize,
    /// Advances on every `Tick`, to animate the working indicator.
    pub spinner: usize,
    /// The report of how protection stands has been asked for once the first screen had its data.
    pub health_asked: bool,
    /// The groups of areas the person made, and why they could not be read, if they could not.
    pub groups: snapall_core::Groups,
    pub groups_error: Option<String>,
    /// Changes this window handed to the background and has not seen end: it follows them, and looks
    /// at the lists again when they have.
    pub watching: Vec<Watched>,
    /// Ticks since the journal was last looked at while something runs.
    pub poll_ticks: u32,
    /// What the changes in flight are, in the present tense: what the screen says is being done.
    pub doing: Vec<String>,
    /// How many ticks (a tenth of a second each) work has been in flight without a pause.
    pub busy_ticks: u32,
    /// What the shell told the UI about the machine, once, at the start.
    pub context: UiContext,
    /// Set when the UI was opened on one file: the screen is then that file's history.
    pub file: Option<FileView>,
    /// The button the mouse is over: what it does is written on the footer line while it is.
    /// What the mouse is over, if it is something that can be clicked: it is lit, and a button says what it does.
    pub hover: Option<Click>,
    /// What was clicked a moment ago, and for how many more ticks it is shown pressed.
    pub pressed: Option<(Click, u8)>,
    /// The parts of the disk still offered for protecting; one leaves the list when it is protected.
    pub offers: Vec<Offer>,
    /// A message to show once the list has been reloaded: the reload clears older notices, and a
    /// confirmation must not be one of them.
    pub after_reload: Option<Notice>,
    /// Which view is showing.
    pub tab: Tab,
    pub health: HealthTab,
    pub activity: ActivityTab,
}

impl Default for State {
    fn default() -> Self {
        Self {
            areas: Load::NotLoaded,
            area_cursor: 0,
            snapshot_cursor: 0,
            marked: BTreeSet::new(),
            focus: Pane::Areas,
            modal: None,
            notice: None,
            quit: false,
            pending: 0,
            spinner: 0,
            health_asked: false,
            groups: snapall_core::Groups::default(),
            groups_error: None,
            watching: Vec::new(),
            poll_ticks: 0,
            doing: Vec::new(),
            busy_ticks: 0,
            context: UiContext::default(),
            file: None,
            hover: None,
            pressed: None,
            offers: Vec::new(),
            after_reload: None,
            tab: Tab::Snapshots,
            health: HealthTab::default(),
            activity: ActivityTab::default(),
        }
    }
}

impl State {
    /// Whether something is going on that the window must keep an eye on: work in flight, a change it
    /// handed to the background, or anything the journal says is running. The clock ticks while it is so.
    pub fn is_following_something(&self) -> bool {
        self.pending > 0
            || self.pressed.is_some()
            || !self.watching.is_empty()
            || self
                .activity
                .entries
                .iter()
                .any(|entry| entry.status == snapall_core::JournalStatus::Running)
    }

    pub fn with_context(context: UiContext) -> Self {
        let file = context.file.clone().map(|target| FileView {
            target,
            versions: Load::NotLoaded,
            cursor: 0,
        });
        Self {
            offers: context.offers.clone(),
            context,
            file,
            ..Self::default()
        }
    }

    /// The versions of the open file, newest first.
    pub fn displayed_versions(&self) -> Vec<&FileVersion> {
        match self.file.as_ref().map(|view| &view.versions) {
            Some(Load::Loaded(list)) => list.iter().rev().collect(),
            _ => Vec::new(),
        }
    }

    pub fn entries(&self) -> &[AreaEntry] {
        match &self.areas {
            Load::Loaded(entries) => entries,
            _ => &[],
        }
    }

    pub fn selected_area(&self) -> Option<&AreaEntry> {
        self.entries().get(self.area_cursor)
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
