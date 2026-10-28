// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window knows: the lists, the cursor, the open prompt, the message, and what it was told about the machine.

mod looked_up;
mod questions;
mod told;
mod window;

pub use looked_up::{AreaEntry, FileTarget, FileView, Load, PointOffer, PointOutcome};
pub use questions::{
    Confirm, GroupsScreen, LimitsEditor, Modal, RestoreStage, StopProtecting, Target,
};
pub use told::{Offer, UiContext};
pub use window::{Notice, State, Watched};
