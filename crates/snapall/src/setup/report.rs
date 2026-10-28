// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What setup did, as a report: each area and what happened to it.

use snapall_core::protecting::coverage::Worth;
use snapall_core::vocabulary::explain::Cleanup;
use snapall_core::AreaName;

/// An area as the report shows it.
pub struct AreaReport {
    pub mount_point: String,
    pub title: String,
    pub protected_by: Option<AreaName>,
    pub worth: Worth,
}

/// What happened.
pub struct Report {
    pub areas: Vec<AreaReport>,
    pub protected_now: Vec<(String, AreaName)>,
    pub declined: Vec<String>,
    /// Things that went wrong, each with the plain-words reason.
    pub failed: Vec<(String, String)>,
    pub cleanup: Cleanup,
    pub cleanup_switched_on: bool,
    pub dry_run: bool,
}

impl Report {
    pub fn succeeded(&self) -> bool {
        self.failed.is_empty()
    }
}
