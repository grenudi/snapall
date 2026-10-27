// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Everything the command line does, by what it is for. One module for each purpose, and `change` for what they
//! all share: a change being made. This file is only the map; each command is a short function in the module
//! that its purpose names.

use snapall_core::SnapshotBackend;

use crate::output::Show;

mod change;
mod deleting;
mod going_back;
mod keeping;
mod looking;
mod protecting;
mod running;
mod taking;
mod window;

pub use deleting::delete;
pub use going_back::{restore, restore_all, undo};
pub use keeping::{groups, limits, quotas, LimitsWanted};
pub use looking::{activity, changes, health, history, list, points, snapshots};
pub use protecting::{setup, unprotect};
pub use running::cancel;
pub use taking::create;
pub use window::tui;

// what the commands share, in one place: the change being made, and the few helpers more than one needs
use change::Change;
use keeping::areas_of_group;
use looking::show_changes;
use window::{absolute, unix_now};

/// What every command works with: the backend (already observed, so every operation is reported),
/// how to show results, and whether tips are allowed.
pub struct Context<'a> {
    pub backend: &'a (dyn SnapshotBackend + Sync),
    pub show: &'a Show,
    pub hints: bool,
}
