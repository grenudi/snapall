// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Health tab: is protection working, how much room does it take, will old snapshots be cleaned
//! up, and what to do about anything that is not.
//!
//! What it shows is decided in [`rows`], a pure function of the report, so it is tested without a
//! terminal. The rest is the cursor, the keys, and the drawing of those rows with the explanation of
//! the one under the cursor beneath them.

use snapall_core::{Facts, Health, Topic};

mod drawing;
mod keys;
mod rows;

pub use rows::*;

/// The report the tab is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthData {
    pub facts: Facts,
    pub report: Health,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HealthTab {
    pub data: Option<HealthData>,
    /// Why the report could not be made, if it could not.
    pub error: Option<String>,
    /// Which row is chosen.
    pub cursor: usize,
}

impl HealthTab {
    pub fn rows(&self) -> Vec<Row> {
        self.data.as_ref().map(rows).unwrap_or_default()
    }

    /// Go to the first row about `topic`: where a click on the status bar lands.
    pub fn select_topic(&mut self, topic: Topic) {
        let rows = self.rows();
        self.cursor = rows
            .iter()
            .position(|row| row.topic == topic)
            .or_else(|| rows.iter().position(|row| row.section == Section::Space))
            .unwrap_or(0);
    }

    /// Keep the cursor on a row after the report changed under it.
    pub fn keep_cursor_in_range(&mut self) {
        let last = self.rows().len().saturating_sub(1);
        self.cursor = self.cursor.min(last);
    }
}

pub use drawing::{render, row_at};
pub use keys::on_key;
