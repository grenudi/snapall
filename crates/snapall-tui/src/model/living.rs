// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's own life: starting, the clock, the pointer and a refusal.

use super::*;

pub(super) fn start(state: &mut State) -> Vec<Effect> {
    match &state.file {
        Some(view) => vec![Effect::LoadFileVersions {
            area: view.target.area.clone(),
            path: view.target.path.clone(),
        }],
        None => vec![Effect::LoadAreas],
    }
}

pub(super) fn refused(state: &mut State, why: String) -> Vec<Effect> {
    settle(state);
    state.notice = Some(Notice::Error(why));
    Vec::new()
}

pub(super) fn tick(state: &mut State) -> Vec<Effect> {
    state.waiting.spinner = state.waiting.spinner.wrapping_add(1);
    state.pointer.pressed = state.pointer.pressed.and_then(|(click, left)| {
        left.checked_sub(1)
            .filter(|left| *left > 0)
            .map(|left| (click, left))
    });
    state.waiting.busy_ticks = if state.waiting.pending > 0 {
        state.waiting.busy_ticks.saturating_add(1)
    } else {
        0
    };
    follow_what_runs(state)
}

pub(super) fn hover(state: &mut State, over: Option<Click>) -> Vec<Effect> {
    state.pointer.hover = over;
    Vec::new()
}

pub(super) fn click(state: &mut State, click: Click) -> Vec<Effect> {
    // shown pressed for a moment, whatever the click then does
    state.pointer.pressed = Some((click, PRESSED_TICKS));
    on_click(state, click)
}
