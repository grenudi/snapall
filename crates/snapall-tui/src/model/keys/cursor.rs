// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Moving through the lists: the cursor, jumping, scrolling, and marking snapshots.

use crate::model::*;

/// Move the cursor of the focused pane. Changing area loads its snapshots on first visit.
pub(crate) fn step(state: &mut State, delta: isize) -> Vec<Effect> {
    match state.snapshots.focus {
        Pane::Areas => {
            let len = state.snapshots.entries().len();
            let next = clamp_step(state.snapshots.area_cursor, delta, len);
            select_area(state, next)
        }
        Pane::Snapshots => {
            let len = state.snapshots.displayed_snapshots().len();
            state.snapshots.snapshot_cursor =
                clamp_step(state.snapshots.snapshot_cursor, delta, len);
            Vec::new()
        }
    }
}

pub(crate) fn jump(state: &mut State, to_end: bool) -> Vec<Effect> {
    match state.snapshots.focus {
        Pane::Areas => {
            let len = state.snapshots.entries().len();
            let target = if to_end { len.saturating_sub(1) } else { 0 };
            select_area(state, target)
        }
        Pane::Snapshots => {
            let len = state.snapshots.displayed_snapshots().len();
            state.snapshots.snapshot_cursor = if to_end { len.saturating_sub(1) } else { 0 };
            Vec::new()
        }
    }
}

pub(crate) fn clamp_step(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    current.saturating_add_signed(delta).min(len - 1)
}

pub(crate) fn select_area(state: &mut State, index: usize) -> Vec<Effect> {
    if index == state.snapshots.area_cursor || index >= state.snapshots.entries().len() {
        return Vec::new();
    }
    state.snapshots.area_cursor = index;
    state.snapshots.snapshot_cursor = 0;
    state.snapshots.marked.clear();
    load_selected_if_needed(state)
}

pub(crate) fn load_selected_if_needed(state: &State) -> Vec<Effect> {
    match state.snapshots.selected_area() {
        Some(entry) if entry.snapshots == Load::NotLoaded => {
            vec![Effect::LoadSnapshots(entry.name.clone())]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn toggle_mark(state: &mut State) {
    if state.snapshots.focus != Pane::Snapshots {
        return;
    }
    let Some(id) = state.snapshots.selected_snapshot().map(|s| s.id) else {
        return;
    };
    if !state.snapshots.marked.remove(&id) {
        state.snapshots.marked.insert(id);
    }
    // Like a file manager: marking moves on to the next row.
    let len = state.snapshots.displayed_snapshots().len();
    state.snapshots.snapshot_cursor = clamp_step(state.snapshots.snapshot_cursor, 1, len);
}

pub(crate) fn on_scroll(state: &mut State, pane: Pane, delta: i32) -> Vec<Effect> {
    if state.modal.is_some() {
        return Vec::new();
    }
    if state.file.is_some() {
        let count = state.displayed_versions().len();
        if let Some(view) = state.file.as_mut() {
            view.cursor = clamp_step(view.cursor, delta as isize, count);
        }
        return Vec::new();
    }
    // The wheel moves the pane under the pointer without taking the keyboard focus from the other.
    let focus = state.snapshots.focus;
    state.snapshots.focus = pane;
    let effects = step(state, delta as isize);
    state.snapshots.focus = focus;
    effects
}
