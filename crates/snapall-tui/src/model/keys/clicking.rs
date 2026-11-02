// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a click does: a button, a row, or the prompt over them.

use super::*;
use crate::model::*;

pub(crate) fn on_click(state: &mut State, click: Click) -> Vec<Effect> {
    match click {
        // A prompt's buttons work only while a prompt is open, and nothing else works then:
        // a click on a button behind a prompt must not be typed into it as a letter.
        Click::Button(
            button @ (Button::Confirm | Button::Cancel | Button::Next | Button::RestoreAll),
        ) => answer_the_prompt(state, button),
        _ if state.modal.is_some() => Vec::new(),
        Click::Tab(tab) => switch_to_tab(state, tab),
        Click::Chip(snapall_core::Topic::Jobs) => show_what_is_running(state),
        Click::Chip(topic) => show_the_topic(state, topic),
        Click::TabRow { index } => choose_the_row_of_the_tab(state, index),
        Click::Button(button) => on_key(state, button.key()),
        Click::Row { index, .. } if state.file.is_some() => choose_a_version(state, index),
        Click::Mark { .. } if state.file.is_some() => Vec::new(),
        Click::Row { pane, index } => choose_a_row(state, pane, index),
        Click::Mark { index } => mark_by_click(state, index),
    }
}

fn answer_the_prompt(state: &mut State, button: Button) -> Vec<Effect> {
    if state.modal.is_some() {
        on_key(state, button.key())
    } else {
        Vec::new()
    }
}

/// What is running is in Activity, with the first of it chosen.
fn show_what_is_running(state: &mut State) -> Vec<Effect> {
    state.tab = Tab::Activity;
    state.activity.cursor = state
        .activity
        .entries
        .iter()
        .position(|entry| entry.status == snapall_core::JournalStatus::Running)
        .unwrap_or(0);
    vec![Effect::LoadHistory]
}

fn show_the_topic(state: &mut State, topic: snapall_core::Topic) -> Vec<Effect> {
    state.tab = Tab::Health;
    state.health.select_topic(topic);
    Vec::new()
}

fn choose_the_row_of_the_tab(state: &mut State, index: usize) -> Vec<Effect> {
    match state.tab {
        Tab::Health if index < state.health.rows().len() => state.health.cursor = index,
        Tab::Activity if index < state.activity.entries.len() => {
            state.activity.cursor = index;
        }
        _ => {}
    }
    Vec::new()
}

/// A row of the file's versions.
fn choose_a_version(state: &mut State, index: usize) -> Vec<Effect> {
    let count = state.displayed_versions().len();
    if let Some(view) = state.file.as_mut() {
        if index < count {
            view.cursor = index;
        }
    }
    Vec::new()
}

/// A row of the areas, or of the snapshots of the chosen area.
fn choose_a_row(state: &mut State, pane: Pane, index: usize) -> Vec<Effect> {
    state.snapshots.focus = pane;
    match pane {
        Pane::Areas if index < state.snapshots.entries().len() => select_area(state, index),
        Pane::Snapshots if index < state.snapshots.displayed_snapshots().len() => {
            state.snapshots.snapshot_cursor = index;
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn mark_by_click(state: &mut State, index: usize) -> Vec<Effect> {
    state.snapshots.focus = Pane::Snapshots;
    if index < state.snapshots.displayed_snapshots().len() {
        state.snapshots.snapshot_cursor = index;
        toggle_mark(state);
        // Marking by click does not move on to the next row, unlike the space key.
        state.snapshots.snapshot_cursor = index;
    }
    Vec::new()
}
