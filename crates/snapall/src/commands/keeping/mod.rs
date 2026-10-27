// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What is kept: how many snapshots, whether their room is counted, and which areas go together.

use super::*;

mod groups;
mod limits;
mod room_counting;

pub use groups::groups;
pub use limits::{limits, LimitsWanted};
pub use room_counting::quotas;

pub(super) use groups::areas_of_group;
