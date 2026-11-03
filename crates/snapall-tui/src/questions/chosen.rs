// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a question about snapshots acts on first: the chosen area, in the snapshots list the person is working in.

use crate::model::{Notice, Pane, State};
use snapall_core::AreaName;

/// The chosen area, when the snapshots list is the one in use. When it is not, the person is told how to get there, in the
/// words of what they were about to do (`to choose what to delete`), and nothing is asked.
pub(crate) fn area_of_the_snapshots_in_use(state: &mut State, to: &str) -> Option<AreaName> {
    let area = state.snapshots.selected_area_name()?;
    if state.snapshots.focus != Pane::Snapshots {
        state.notice = Some(Notice::Info(format!(
            "move to the snapshots pane (right arrow) {to}"
        )));
        return None;
    }
    Some(area)
}
