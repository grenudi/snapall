// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Making a change one at a time: what is held while it is made, and what it is told when another is already being made.

/// Another snapall is making a change, so this one must wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Busy {
    pub pid: u32,
    /// What it is doing, as it said when it started.
    pub doing: String,
    /// When it started, if known.
    pub since: Option<String>,
}

impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Another snapall is making a change right now ({}",
            self.doing
        )?;
        if let Some(since) = &self.since {
            write!(f, ", since {since}")?;
        }
        write!(
            f,
            ", process {}). Wait for it to finish, then try again.",
            self.pid
        )
    }
}

/// Held while a change is made; letting go of it lets the next one start.
pub type Hold = Box<dyn std::any::Any + Send>;
