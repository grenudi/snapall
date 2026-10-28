// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tabs of the window, and which one is showing. A tab shows the state of things (the snapshots, how protection is doing,
//! what snapall did); what changes things (create, restore, protect) is a prompt that opens over any
//! of them.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Snapshots,
    Health,
    Activity,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Snapshots, Tab::Health, Tab::Activity];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Snapshots => "Snapshots",
            Tab::Health => "Health",
            Tab::Activity => "Activity",
        }
    }

    /// The key that goes to this tab: 1, 2, 3.
    pub fn key(self) -> char {
        match self {
            Tab::Snapshots => '1',
            Tab::Health => '2',
            Tab::Activity => '3',
        }
    }

    pub fn from_key(key: char) -> Option<Tab> {
        Tab::ALL.into_iter().find(|tab| tab.key() == key)
    }

    /// What the tab is for, in a sentence: the footer says it when the mouse is over the tab.
    pub fn purpose(self) -> &'static str {
        match self {
            Tab::Snapshots => "Your protected areas and their snapshots: take, delete and restore",
            Tab::Health => "Is protection working, how much room it takes, and what to do about it",
            Tab::Activity => "What snapall has done, from here and from commands, and how it went",
        }
    }
}
