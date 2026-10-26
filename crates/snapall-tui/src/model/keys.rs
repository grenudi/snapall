// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What each key and click does, and how a prompt answers one. A key means what the screen in front of it means.

use crate::model::*;

/// Switching tabs (1, 2, 3) and the keys of the tab that is showing. `None` is a key the window itself
/// handles, whatever is showing: quit, help, take a snapshot of everything, protect, restore points.
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
            vec![Effect::LoadConfigs, Effect::LoadHealth, Effect::LoadHistory]
        }
        Key::Tab => {
            state.focus = match state.focus {
                Pane::Configs => Pane::Snapshots,
                Pane::Snapshots => Pane::Configs,
            };
            Vec::new()
        }
        Key::Left | Key::Char('h') => {
            state.focus = Pane::Configs;
            Vec::new()
        }
        Key::Right | Key::Char('l') => {
            state.focus = Pane::Snapshots;
            Vec::new()
        }
        Key::Up | Key::Char('k') => step(state, -1),
        Key::Down | Key::Char('j') => step(state, 1),
        Key::Home | Key::Char('g') => jump(state, false),
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
        Key::Char('R') => {
            state.modal = Some(Modal::Points { index: 0 });
            state.points = None;
            state.point_preview = None;
            vec![Effect::LoadPoints]
        }
        _ => Vec::new(),
    }
}

/// What a prompt becomes after a key, and the work that key starts.
pub(crate) struct Reaction {
    /// The prompt as it now is, or `None` if it closes.
    pub(crate) modal: Option<Modal>,
    pub(crate) effects: Vec<Effect>,
}

impl Reaction {
    /// The prompt stays open, as given (possibly changed by the key).
    pub(crate) fn stays(modal: Modal) -> Self {
        Self {
            modal: Some(modal),
            effects: Vec::new(),
        }
    }

    pub(crate) fn closes() -> Self {
        Self {
            modal: None,
            effects: Vec::new(),
        }
    }

    /// The prompt stays open and work starts (moving to another restore point looks it up).
    pub(crate) fn stays_and_starts(modal: Modal, effects: Vec<Effect>) -> Self {
        Self {
            modal: Some(modal),
            effects,
        }
    }

    /// The prompt is answered: it closes and the work it asked for starts.
    pub(crate) fn closes_and_starts(effects: Vec<Effect>) -> Self {
        Self {
            modal: None,
            effects,
        }
    }
}

pub(crate) fn on_modal_key(state: &mut State, key: Key) -> Vec<Effect> {
    let Some(modal) = state.modal.take() else {
        return Vec::new();
    };
    let reaction = match modal {
        Modal::Help { page } => reading_help(key, page),
        Modal::Protect { index } => protecting(state, key, index),
        Modal::Points { index } => browsing_points(state, key, index),
        Modal::Job { unit } => controlling_job(state, key, unit),
        Modal::Quotas { turn_on } => confirming_quotas(state, key, turn_on),
        Modal::StopChange { id, what } => confirming_stop(state, key, id, what),
        Modal::Describe { target, input, .. } => describing(state, key, target, input),
        Modal::ConfirmDelete { config, ids } => confirming_delete(state, key, config, ids),
        Modal::Restore {
            config,
            id,
            path,
            stage,
        } => confirming_restore(state, key, config, id, path, stage),
    };
    state.modal = reaction.modal;
    reaction.effects
}

/// Move the cursor of the focused pane. Changing config loads its snapshots on first visit.
pub(crate) fn step(state: &mut State, delta: isize) -> Vec<Effect> {
    match state.focus {
        Pane::Configs => {
            let len = state.entries().len();
            let next = clamp_step(state.config_cursor, delta, len);
            select_config(state, next)
        }
        Pane::Snapshots => {
            let len = state.displayed_snapshots().len();
            state.snapshot_cursor = clamp_step(state.snapshot_cursor, delta, len);
            Vec::new()
        }
    }
}

pub(crate) fn jump(state: &mut State, to_end: bool) -> Vec<Effect> {
    match state.focus {
        Pane::Configs => {
            let len = state.entries().len();
            let target = if to_end { len.saturating_sub(1) } else { 0 };
            select_config(state, target)
        }
        Pane::Snapshots => {
            let len = state.displayed_snapshots().len();
            state.snapshot_cursor = if to_end { len.saturating_sub(1) } else { 0 };
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

pub(crate) fn select_config(state: &mut State, index: usize) -> Vec<Effect> {
    if index == state.config_cursor || index >= state.entries().len() {
        return Vec::new();
    }
    state.config_cursor = index;
    state.snapshot_cursor = 0;
    state.marked.clear();
    load_selected_if_needed(state)
}

pub(crate) fn load_selected_if_needed(state: &State) -> Vec<Effect> {
    match state.selected_config() {
        Some(entry) if entry.snapshots == Load::NotLoaded => {
            vec![Effect::LoadSnapshots(entry.name.clone())]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn toggle_mark(state: &mut State) {
    if state.focus != Pane::Snapshots {
        return;
    }
    let Some(id) = state.selected_snapshot().map(|s| s.id) else {
        return;
    };
    if !state.marked.remove(&id) {
        state.marked.insert(id);
    }
    // Like a file manager: marking moves on to the next row.
    let len = state.displayed_snapshots().len();
    state.snapshot_cursor = clamp_step(state.snapshot_cursor, 1, len);
}

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
                Pane::Configs if index < state.entries().len() => select_config(state, index),
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
    let focus = state.focus;
    state.focus = pane;
    let effects = step(state, delta as isize);
    state.focus = focus;
    effects
}
