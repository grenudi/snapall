// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Going back: to a snapshot, to a restore point, or to before a change.

use super::*;

mod restoring;
mod restoring_a_point;
mod undoing;

pub use restoring::restore;
pub use restoring_a_point::restore_all;
pub use undoing::undo;
