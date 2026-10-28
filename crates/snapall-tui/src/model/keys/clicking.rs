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
        ) => {
            if state.modal.is_some() {
                on_key(state, button.key())
            } else {
                Vec::new()
            }
        }
        _ if state.modal.is_some() => Vec::new(),
        Click::Tab(tab) => {
            state.tab = tab;
            if tab == Tab::Activity {
                vec![Effect::LoadHistory]
            } else {
                Vec::new()
            }
        }
        Click::Chip(snapall_core::Topic::Jobs) => {
            // what is running is in Activity, with the first of it chosen
            state.tab = Tab::Activity;
            state.activity.cursor = state
                .activity
                .entries
                .iter()
                .position(|entry| entry.status == snapall_core::JournalStatus::Running)
                .unwrap_or(0);
            vec![Effect::LoadHistory]
        }
        Click::Chip(topic) => {
            state.tab = Tab::Health;
            state.health.select_topic(topic);
            Vec::new()
        }
        Click::TabRow { index } => {
            match state.tab {
                Tab::Health if index < state.health.rows().len() => state.health.cursor = index,
                Tab::Activity if index < state.activity.entries.len() => {
                    state.activity.cursor = index;
                }
                _ => {}
            }
            Vec::new()
        }
        Click::Button(button) => on_key(state, button.key()),
        Click::Row { index, .. } if state.file.is_some() => {
            let count = state.displayed_versions().len();
            if let Some(view) = state.file.as_mut() {
                if index < count {
                    view.cursor = index;
                }
            }
            Vec::new()
        }
        Click::Mark { .. } if state.file.is_some() => Vec::new(),
        Click::Row { pane, index } => {
            state.focus = pane;
            match pane {
                Pane::Areas if index < state.entries().len() => select_area(state, index),
                Pane::Snapshots if index < state.displayed_snapshots().len() => {
                    state.snapshot_cursor = index;
                    Vec::new()
                }
                _ => Vec::new(),
            }
        }
        Click::Mark { index } => {
            state.focus = Pane::Snapshots;
            if index < state.displayed_snapshots().len() {
                state.snapshot_cursor = index;
                toggle_mark(state);
                // Marking by click does not move on to the next row, unlike the space key.
                state.snapshot_cursor = index;
            }
            Vec::new()
        }
    }
}
