// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deleting snapshots: asking first, and what comes of it.

use crate::model::*;
use snapall_core::Act;

/// Deleting cannot be undone, so it asks: `y` or Enter confirms, `n` or Esc cancels.
pub(crate) fn confirming_delete(
    _state: &mut State,
    key: Key,
    area: AreaName,
    ids: Vec<SnapshotId>,
) -> Reaction {
    match key {
        Key::Char('y' | 'Y') | Key::Enter => {
            let list: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
            let lines = vec![
                format!("Delete {} snapshot(s) of {area} for good: {}.", ids.len(), list.join(", ")),
                "Only these saved moments go: the files are not touched. A deleted snapshot cannot be brought back.".to_owned(),
            ];
            ask_first(
                Act::DeleteSnapshots,
                None,
                lines,
                vec![Effect::Delete { area, ids }],
            )
        }
        Key::Char('n' | 'N') | Key::Esc | Key::CtrlC => Reaction::closes(),
        _ => Reaction::stays(Modal::ConfirmDelete { area, ids }),
    }
}

pub(crate) fn open_delete(state: &mut State) -> Vec<Effect> {
    let Some(area) = state.selected_area().map(|c| c.name.clone()) else {
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
    state.modal = Some(Modal::ConfirmDelete { area, ids });
    Vec::new()
}

pub(crate) fn on_deleted(
    state: &mut State,
    area: &AreaName,
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
        Notice::Info(format!("deleted {} from {area}", deleted.join(", ")))
    } else {
        Notice::Error(format!("{area}: {}", failures.join("; ")))
    });
    state.marked.clear();
    vec![Effect::LoadSnapshots(area.clone())]
}
