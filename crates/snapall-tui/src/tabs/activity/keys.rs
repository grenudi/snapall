// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the keys do on the Activity tab: move along the changes, and ask to stop the chosen one.

use crate::model::{Effect, Key, State};
use snapall_core::JournalStatus;

/// The keys of this tab; `None` is a key the window handles.
pub fn on_key(state: &mut State, key: Key) -> Option<Vec<Effect>> {
    let last = state.activity.entries.len().saturating_sub(1);
    let tab = &mut state.activity;
    match key {
        Key::Up | Key::Char('k') => tab.cursor = tab.cursor.saturating_sub(1),
        Key::Down | Key::Char('j') => tab.cursor = (tab.cursor + 1).min(last),
        Key::Home => tab.cursor = 0,
        Key::End | Key::Char('G') => tab.cursor = last,
        Key::Char('x') => ask_to_stop_the_chosen(state),
        _ => return None,
    }
    Some(Vec::new())
}

/// `x` on a change that is running opens the question; on anything else it says what it is for.
pub(super) fn ask_to_stop_the_chosen(state: &mut State) {
    let chosen = state.activity.entries.get(state.activity.cursor).cloned();
    match chosen {
        Some(entry) if entry.status == JournalStatus::Running && entry.id.is_some() => {
            let id = entry.id.unwrap_or_default();
            state.modal = Some(crate::model::Modal::StopChange {
                id,
                what: entry.what,
            });
        }
        _ => {
            state.notice = Some(crate::model::Notice::Info(
                "x asks a change that is running to stop; this one is not running".into(),
            ));
        }
    }
}
