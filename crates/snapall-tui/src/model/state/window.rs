// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window at one moment: its lists, cursor, open question, message, and what it is waiting for.

use super::*;
use crate::model::*;
use crate::tabs::snapshots::SnapshotsTab;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Info(String),
    Error(String),
}

impl Notice {
    /// A failure, said the one way: `Could not change the limits of home: Access denied`. `what` completes "could not".
    pub fn could_not(what: impl std::fmt::Display, why: impl std::fmt::Display) -> Notice {
        Notice::Error(format!("Could not {what}: {why}"))
    }
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
    pub snapshots: SnapshotsTab,
    pub modal: Option<Modal>,
    pub notice: Option<Notice>,
    pub quit: bool,
    pub waiting: Waiting,
    /// The groups of areas the person made, and why they could not be read, if they could not.
    pub groups: snapall_core::Groups,
    pub groups_error: Option<String>,
    /// What the shell told the UI about the machine, once, at the start.
    pub context: UiContext,
    /// Set when the UI was opened on one file: the screen is then that file's history.
    pub file: Option<FileView>,
    pub pointer: Pointer,
    /// The parts of the disk still offered for protecting; one leaves the list when it is protected.
    pub offers: Vec<Offer>,
    /// Which view is showing.
    pub tab: Tab,
    pub health: HealthTab,
    pub activity: ActivityTab,
}

impl Default for State {
    fn default() -> Self {
        Self {
            snapshots: SnapshotsTab::default(),
            modal: None,
            notice: None,
            quit: false,
            waiting: Waiting::default(),
            groups: snapall_core::Groups::default(),
            groups_error: None,
            context: UiContext::default(),
            file: None,
            pointer: Pointer::default(),
            offers: Vec::new(),
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
        self.waiting.pending > 0
            || self.pointer.pressed.is_some()
            || !self.waiting.watching.is_empty()
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
}
