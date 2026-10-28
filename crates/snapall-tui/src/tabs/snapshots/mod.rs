// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Snapshots tab: the protected areas on the left and the chosen area's snapshots on the right.

mod arriving;
mod drawing;

pub(crate) use arriving::{on_areas_loaded, on_created, on_snapshots_loaded};
pub(crate) use drawing::{areas_split, pane_block, render_areas, render_snapshots};
