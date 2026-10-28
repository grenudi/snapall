// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Everything the command line does, by what it is for. One module for each purpose, and `change` for what they
//! all share: a change being made. This file is only the map; each command is a short function in the module
//! that its purpose names.

use snapall_core::SnapshotBackend;

use crate::output::Show;

mod about;
mod asked_for;
mod change;
mod deleting;
mod going_back;
mod keeping;
mod looking;
mod protecting;
mod running;
mod taking;
mod window;

pub use asked_for::carry_out;

// what the commands share, in one place: the change being made, and the few helpers more than one needs
use change::Change;
use keeping::areas_of_group;
use looking::show_changes;
use window::{absolute, unix_now};

/// What every command works with: the backend (already observed, so every operation is reported), the observer, how
/// to show results, and how the program was invoked.
pub struct Context<'a> {
    pub backend: &'a (dyn SnapshotBackend + Sync),
    pub show: &'a Show,
    pub observer: &'a (dyn snapall_core::Observer + Send + Sync),
    pub hints: bool,
    pub json: bool,
    pub no_dbus: bool,
}
