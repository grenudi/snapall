// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window does when something happens: its state, what can happen to it, and what it asks for in return.
//!
//! Nothing here touches a terminal or a backend. `update` mutates the state and returns
//! the *effects* the shell should perform; the shell reports what happened as new messages.

use std::time::Duration;

use crate::tab::Tab;
use crate::tabs::activity::ActivityTab;
use crate::tabs::health::{HealthData, HealthTab};
use snapall_core::{
    AreaName, Description, FileVersion, Preview, Restored, SnapshotId, SnapshotInfo,
};

mod changed;
mod following;
mod input;
mod keys;
mod living;
mod looked;
mod messages;
mod state;
mod updating;

pub(crate) use crate::file_history::{on_file_key, on_file_versions_loaded};
pub(crate) use crate::questions::asking::*;
pub(crate) use crate::tabs::snapshots::{on_areas_loaded, on_created, on_snapshots_loaded};
pub(crate) use following::*;
pub use input::*;
pub(crate) use keys::*;
pub use messages::*;
pub use state::*;
pub use updating::update;
use updating::{handle, settle};
