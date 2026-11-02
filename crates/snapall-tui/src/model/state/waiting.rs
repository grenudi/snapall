// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window is waiting for: work in flight, the changes it handed to the background, and for how long.

use super::{Notice, Watched};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Waiting {
    /// Effects handed to the shell whose results have not come back. While it is above zero the
    /// screen says "working" and the keys that would start more work are refused. A shell that
    /// runs effects instantly (the tests) never sees it above zero between messages.
    pub pending: usize,
    /// Advances on every `Tick`, to animate the working indicator.
    pub spinner: usize,
    /// Changes this window handed to the background and has not seen end: it follows them, and looks
    /// at the lists again when they have.
    pub watching: Vec<Watched>,
    /// Ticks since the journal was last looked at while something runs.
    pub poll_ticks: u32,
    /// What the changes in flight are, in the present tense: what the screen says is being done.
    pub doing: Vec<String>,
    /// How many ticks (a tenth of a second each) work has been in flight without a pause.
    pub busy_ticks: u32,
    /// A message to show once the list has been reloaded: the reload clears older notices, and a
    /// confirmation must not be one of them.
    pub after_reload: Option<Notice>,
}
