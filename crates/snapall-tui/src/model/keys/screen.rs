// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a key does on the screen the person is looking at: the tab's own keys first, then the keys every screen shares.

use super::*;
use crate::model::*;

pub(crate) fn on_tab_key(state: &mut State, key: Key) -> Option<Vec<Effect>> {
    if let Key::Char(c) = key {
        if let Some(tab) = Tab::from_key(c) {
            state.tab = tab;
            // another snapall may have done something since: read the journal again
            return Some(if tab == Tab::Activity {
                vec![Effect::LoadHistory]
            } else {
                Vec::new()
            });
        }
    }
    let own = match state.tab {
        Tab::Snapshots => return None,
        Tab::Health => crate::tabs::health::on_key(state, key),
        Tab::Activity => crate::tabs::activity::on_key(state, key),
    };
    // The snapshot lists are not showing, so the keys that act on them do nothing here.
    own.or_else(|| {
        matches!(
            key,
            Key::Tab | Key::Left | Key::Right | Key::Char('h' | 'l' | 'c' | 'd' | 'u' | ' ')
        )
        .then(Vec::new)
    })
}

pub(crate) fn on_key(state: &mut State, key: Key) -> Vec<Effect> {
    if state.modal.is_some() {
        return on_modal_key(state, key);
    }
    // Moving around and reading stay available while snapper works; starting more work does not.
    if state.pending > 0 && matches!(key, Key::Char('r' | 'c' | 'a' | 'd' | 'u' | 'p' | 'R')) {
        state.notice = Some(Notice::Error(
            "busy: wait for the current operation to finish".into(),
        ));
        return Vec::new();
    }
    if state.file.is_some() {
        return on_file_key(state, key);
    }
    if let Some(effects) = on_tab_key(state, key) {
        return effects;
    }
    match key {
        Key::CtrlC | Key::Char('q') => {
            state.quit = true;
            Vec::new()
        }
        Key::Char('?') => {
            state.modal = Some(Modal::Help { page: 0 });
            Vec::new()
        }
        Key::Char('r') => {
            state.notice = Some(Notice::Info("reloading".into()));
            vec![Effect::LoadAreas, Effect::LoadHealth, Effect::LoadHistory]
        }
        Key::Tab => {
            state.focus = match state.focus {
                Pane::Areas => Pane::Snapshots,
                Pane::Snapshots => Pane::Areas,
            };
            Vec::new()
        }
        Key::Left | Key::Char('h') => {
            state.focus = Pane::Areas;
            Vec::new()
        }
        Key::Right | Key::Char('l') => {
            state.focus = Pane::Snapshots;
            Vec::new()
        }
        Key::Up | Key::Char('k') => step(state, -1),
        Key::Down | Key::Char('j') => step(state, 1),
        Key::Home => jump(state, false),
        Key::End | Key::Char('G') => jump(state, true),
        Key::Char(' ') => {
            toggle_mark(state);
            Vec::new()
        }
        Key::Char('c') => open_describe(state, false),
        Key::Char('a') => open_describe(state, true),
        Key::Char('d') => open_delete(state),
        Key::Char('u') => open_restore(state),
        Key::Char('p') => {
            state.modal = Some(Modal::Protect { index: 0 });
            Vec::new()
        }
        Key::Char('g') => {
            state.modal = Some(Modal::Groups(GroupsScreen::List { cursor: 0 }));
            Vec::new()
        }
        Key::Char('R') => {
            state.modal = Some(Modal::Points {
                index: 0,
                points: None,
                preview: None,
            });
            vec![Effect::LoadPoints]
        }
        _ => Vec::new(),
    }
}
