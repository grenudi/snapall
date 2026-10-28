// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where a click lands: the same layout the drawing used, so a click can only land on what was drawn.

use crate::view::*;

// ---- clicks ---------------------------------------------------------------------------------

/// The first visible row of a list of `len` rows, `rows` high, whose selection is `cursor`. The
/// panes are redrawn from scratch every frame, so the window depends on nothing but these three
/// numbers: the selection is brought into view by the least scroll from the top.
pub(crate) fn window_start(cursor: usize, rows: usize) -> usize {
    if rows > 0 && cursor >= rows {
        cursor + 1 - rows
    } else {
        0
    }
}

pub(crate) fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// The pane a terminal cell belongs to, for the wheel.
pub fn pane_at(state: &State, size: Rect, col: u16, row: u16) -> Option<Pane> {
    let areas = layout(state, size);
    if contains(areas.configs, col, row) {
        Some(Pane::Areas)
    } else if contains(areas.snapshots, col, row) {
        Some(Pane::Snapshots)
    } else {
        None
    }
}

/// What a left click at (`col`, `row`) hits, for a terminal of this size, or `None`.
///
/// With a prompt open only the prompt's own buttons answer; behind it, nothing does.
pub fn hit_test(state: &State, size: Rect, col: u16, row: u16) -> Option<Click> {
    if let Some(modal) = &state.modal {
        let (_, _, buttons) = modal_geometry(modal, state, size);
        return buttons
            .into_iter()
            .find(|(rect, _, _)| contains(*rect, col, row))
            .map(|(_, button, _)| Click::Button(button));
    }
    let areas = layout(state, size);
    if state.file.is_none() {
        if let Some((_, tab)) = tab_rects(areas.header)
            .into_iter()
            .find(|(rect, _)| contains(*rect, col, row))
        {
            return Some(Click::Tab(tab));
        }
    }
    if let Some((_, topic)) = status_bar::chip_rects(areas.status, &status_bar::chips(state))
        .into_iter()
        .find(|(rect, _)| contains(*rect, col, row))
    {
        return Some(Click::Chip(topic));
    }
    if let Some((_, button, _, _)) = footer_buttons(areas.keys, state.file.is_some())
        .into_iter()
        .find(|(rect, _, _, _)| contains(*rect, col, row))
    {
        // a button that does nothing here is not one to click
        return (!is_disabled(state, button)).then_some(Click::Button(button));
    }
    if state.file.is_none() && state.tab != Tab::Snapshots {
        let index = match state.tab {
            Tab::Health => tabs::health::row_at(state, areas.body, row),
            Tab::Activity => tabs::activity::row_at(state, areas.body, row),
            Tab::Snapshots => None,
        };
        return index.map(|index| Click::TabRow { index });
    }
    let block = Block::default().borders(Borders::ALL);
    let (configs, groups) = areas_split(state, block.inner(areas.configs));
    // the groups under the areas open the groups, like the button
    if groups.is_some_and(|groups| contains(groups, col, row)) {
        return Some(Click::Button(Button::Groups));
    }
    if state.file.is_none() && contains(configs, col, row) && !state.entries().is_empty() {
        let index =
            window_start(state.area_cursor, configs.height as usize) + (row - configs.y) as usize;
        return (index < state.entries().len()).then_some(Click::Row {
            pane: Pane::Areas,
            index,
        });
    }
    let snapshots = block.inner(areas.snapshots);
    if let Some(view) = &state.file {
        // One header line, then a row per version, newest first.
        if contains(snapshots, col, row) && row > snapshots.y {
            let visible = (snapshots.height as usize).saturating_sub(1);
            let index = window_start(view.cursor, visible) + (row - snapshots.y - 1) as usize;
            if index < state.displayed_versions().len() {
                return Some(Click::Row {
                    pane: Pane::Snapshots,
                    index,
                });
            }
        }
        return None;
    }
    // One header line, then the rows.
    if contains(snapshots, col, row) && row > snapshots.y {
        let visible = (snapshots.height as usize).saturating_sub(1);
        let index = window_start(state.snapshot_cursor, visible) + (row - snapshots.y - 1) as usize;
        if index < state.displayed_snapshots().len() {
            // The first two cells are the mark column and its spacing.
            return Some(if col < snapshots.x.saturating_add(2) {
                Click::Mark { index }
            } else {
                Click::Row {
                    pane: Pane::Snapshots,
                    index,
                }
            });
        }
    }
    None
}
