// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window is told about the machine, once, at the start: its tips, what could be protected, whether it may change things.

use super::*;

/// A part of the disk that has no snapshots yet, offered for protecting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// Where it is mounted: `/home`.
    pub mount_point: String,
    /// What a person calls it: `Home`.
    pub title: String,
    /// What protecting it means, in plain words: what it holds, whether it is worth it, what will and
    /// will not happen.
    pub summary: String,
    pub recommended: bool,
}

/// What the UI is told about the machine it runs on, once, at the start. The model never reads
/// the machine; the shell does, and hands over plain text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiContext {
    /// A one-line tip (for example that snapshots cannot be booted), shown until something else
    /// needs the line.
    pub boot_tip: Option<String>,
    /// How to restore the *system itself*, shown instead of a confirm button for the system root.
    pub restore_route: String,
    /// Open on this file's history instead of on all areas.
    pub file: Option<FileTarget>,
    /// The parts of the disk that have no snapshots yet and could have, best first.
    pub offers: Vec<Offer>,
    /// Whether snapall may change how snapshots are kept (it was started with `sudo`).
    pub administrator: bool,
    /// One line for the footer when something worth protecting is not: it says what to press.
    pub protect_hint: Option<String>,
}
