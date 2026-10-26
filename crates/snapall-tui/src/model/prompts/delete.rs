// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deleting snapshots: asking first, and what comes of it.

use crate::model::*;

/// Deleting cannot be undone, so it asks: `y` or Enter confirms, `n` or Esc cancels.
pub(crate) fn confirming_delete(
    state: &mut State,
    key: Key,
    config: ConfigName,
    ids: Vec<SnapshotId>,
) -> Reaction {
    match key {
        Key::Char('y' | 'Y') | Key::Enter => {
            state.notice = Some(Notice::Info(format!("deleting {} snapshot(s)", ids.len())));
            Reaction::closes_and_starts(vec![Effect::Delete { config, ids }])
        }
        Key::Char('n' | 'N') | Key::Esc | Key::CtrlC => Reaction::closes(),
        _ => Reaction::stays(Modal::ConfirmDelete { config, ids }),
    }
}

pub(crate) fn open_delete(state: &mut State) -> Vec<Effect> {
    let Some(config) = state.selected_config().map(|c| c.name.clone()) else {
        return Vec::new();
    };
    if state.focus != Pane::Snapshots {
        state.notice = Some(Notice::Info(
            "move to the snapshots pane (right arrow) to choose what to delete".into(),
        ));
        return Vec::new();
    }
    let ids: Vec<SnapshotId> = if state.marked.is_empty() {
        state
            .selected_snapshot()
            .map(|s| s.id)
            .into_iter()
            .collect()
    } else {
        state.marked.iter().copied().collect()
    };
    if ids.is_empty() {
        state.notice = Some(Notice::Error("no snapshot to delete".into()));
        return Vec::new();
    }
    state.modal = Some(Modal::ConfirmDelete { config, ids });
    Vec::new()
}

pub(crate) fn on_deleted(
    state: &mut State,
    config: &ConfigName,
    results: Vec<(SnapshotId, Result<(), String>)>,
) -> Vec<Effect> {
    let deleted: Vec<String> = results
        .iter()
        .filter(|(_, r)| r.is_ok())
        .map(|(id, _)| format!("#{id}"))
        .collect();
    let failures: Vec<String> = results
        .iter()
        .filter_map(|(id, r)| r.as_ref().err().map(|e| format!("#{id}: {e}")))
        .collect();
    state.notice = Some(if failures.is_empty() {
        Notice::Info(format!("deleted {} from {config}", deleted.join(", ")))
    } else {
        Notice::Error(format!("{config}: {}", failures.join("; ")))
    });
    state.marked.clear();
    vec![Effect::LoadSnapshots(config.clone())]
}
