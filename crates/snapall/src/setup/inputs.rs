// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the setup flow is given: the facts about the machine, what was asked for, and how it talks and acts.

use snapall_core::vocabulary::explain::Cleanup;
use std::error::Error;

pub struct Facts {
    /// The text of `/proc/self/mountinfo`.
    pub mountinfo: String,
    pub is_root: bool,
    pub cleanup: Cleanup,
    /// When the cleanup job runs, in words, from its unit file; empty if that is not known.
    pub cleanup_schedule: Vec<String>,
}

pub struct Options {
    /// Protect this part of the disk instead of the recommended ones.
    pub area: Option<String>,
    /// Accept what snapall recommends without asking.
    pub yes: bool,
    /// Explain and show what would be done, and do nothing.
    pub dry_run: bool,
}

/// How the flow talks to the person and acts on the system.
pub trait Talk {
    fn say(&mut self, text: &str);
    /// A yes/no question whose recommended answer is yes.
    fn ask(&mut self, question: &str) -> Result<bool, Box<dyn Error>>;
    fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>>;
}
