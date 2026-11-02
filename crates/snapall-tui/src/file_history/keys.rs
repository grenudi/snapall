// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the keys do on the file history screen: move along the versions, reload them, and put one back.

use crate::model::*;
use snapall_core::FileState;

pub(crate) fn on_file_key(state: &mut State, key: Key) -> Vec<Effect> {
    let count = state.displayed_versions().len();
    let Some(view) = state.file.as_mut() else {
        return Vec::new();
    };
    match key {
        Key::CtrlC | Key::Char('q') => quit(state),
        Key::Char('?') => open_help(state),
        Key::Up | Key::Char('k') => {
            view.cursor = clamp_step(view.cursor, -1, count);
            Vec::new()
        }
        Key::Down | Key::Char('j') => {
            view.cursor = clamp_step(view.cursor, 1, count);
            Vec::new()
        }
        Key::Home | Key::Char('g') => {
            view.cursor = 0;
            Vec::new()
        }
        Key::End | Key::Char('G') => {
            view.cursor = count.saturating_sub(1);
            Vec::new()
        }
        Key::Char('r') => {
            state.notice = Some(Notice::Info("reloading".into()));
            vec![Effect::LoadFileVersions {
                area: view.target.area.clone(),
                path: view.target.path.clone(),
            }]
        }
        Key::Enter | Key::Char('u') => open_file_restore(state),
        _ => Vec::new(),
    }
}

pub(crate) fn open_file_restore(state: &mut State) -> Vec<Effect> {
    let Some(view) = state.file.as_ref() else {
        return Vec::new();
    };
    let (area, path) = (view.target.area.clone(), view.target.path.clone());
    let cursor = view.cursor;
    let Some(version) = state
        .displayed_versions()
        .get(cursor)
        .map(|v| (v.snapshot.id, v.state))
    else {
        state.notice = Some(Notice::Error("no version to restore".into()));
        return Vec::new();
    };
    let (id, file_state) = version;
    if !file_state.restorable() {
        state.notice = Some(Notice::Info(match file_state {
            FileState::Same => format!("the file is already the same as in #{id}"),
            _ => format!("the file did not exist in #{id}: there is nothing to restore"),
        }));
        return Vec::new();
    }
    state.modal = Some(Modal::Restore {
        area: area.clone(),
        id,
        path: Some(path.clone()),
        stage: RestoreStage::Loading,
        offer: None,
    });
    vec![Effect::LoadChanges {
        area,
        id,
        path: Some(path),
    }]
}
