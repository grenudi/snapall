// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The journal's changes as the Activity tab holds them: each one, and which is chosen.

use snapall_core::{JournalEntry, JournalStatus, Source};
use std::collections::HashSet;
use std::time::Duration;

pub(super) const KEPT: usize = 200;

/// One thing snapall did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// Which change it is in the journal, so that it is shown once however often it is read.
    pub id: Option<String>,
    /// When it finished (or started, if it has not), as the machine's clock wrote it.
    pub at: Option<String>,
    /// What it was, in a sentence: `Took a snapshot of home, root`.
    pub what: String,
    pub status: JournalStatus,
    /// What came of it, or why it failed.
    pub detail: String,
    pub took: Option<Duration>,
    pub source: Option<Source>,
    /// The process doing it, while it runs.
    pub pid: Option<u32>,
    /// How far it has come, while it runs.
    pub progress: Option<snapall_core::JournalProgress>,
    /// How to get back from it, or that it cannot be undone.
    pub way_back: Option<snapall_core::Rollback>,
}

impl Activity {
    pub fn from_entry(entry: &JournalEntry) -> Activity {
        Activity {
            id: Some(entry.id.clone()),
            at: entry
                .finished_at
                .clone()
                .or_else(|| entry.started_at.clone()),
            what: entry.what.clone(),
            status: entry.status,
            detail: entry.detail.clone(),
            took: entry.took_ms.map(Duration::from_millis),
            source: entry.source,
            pid: entry.pid,
            progress: entry.progress.clone(),
            way_back: entry.rollback.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityTab {
    /// Newest first.
    pub entries: Vec<Activity>,
    pub cursor: usize,
}

impl ActivityTab {
    /// This window did something: it is shown at once, before the journal has been read again.
    pub fn record(&mut self, activity: Activity) {
        self.entries.insert(0, activity);
        self.entries.truncate(KEPT);
        self.cursor = 0; // the newest is what a person looks for
    }

    /// The journal was read. It is the record; what this window did that the journal does not have (no
    /// journal on this machine) stays, newest.
    pub fn merge_history(&mut self, history: &[JournalEntry]) {
        let known: HashSet<&str> = history.iter().map(|entry| entry.id.as_str()).collect();
        let mut merged: Vec<Activity> = std::mem::take(&mut self.entries)
            .into_iter()
            .filter(|own| own.id.as_deref().is_none_or(|id| !known.contains(id)))
            .collect();
        merged.extend(history.iter().map(Activity::from_entry));
        merged.truncate(KEPT);
        self.entries = merged;
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }
}
