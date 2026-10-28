// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lists of areas and snapshots: what happens when they arrive.

use crate::model::*;

pub(crate) fn on_areas_loaded(
    state: &mut State,
    result: Result<Vec<AreaName>, String>,
) -> Vec<Effect> {
    match result {
        Err(error) => {
            state.areas = Load::Failed(error.clone());
            state.area_cursor = 0;
            state.after_reload = None;
            state.notice = Some(Notice::Error(error));
            Vec::new()
        }
        Ok(mut names) => {
            let previous = state.selected_area().map(|c| c.name.clone());
            names.sort();
            names.dedup();
            state.area_cursor = previous
                .and_then(|p| names.iter().position(|n| *n == p))
                .unwrap_or(0);
            state.areas = Load::Loaded(
                names
                    .into_iter()
                    .map(|name| AreaEntry {
                        name,
                        snapshots: Load::NotLoaded,
                    })
                    .collect(),
            );
            state.snapshot_cursor = 0;
            state.marked.clear();
            if matches!(state.notice, Some(Notice::Info(_))) {
                state.notice = None;
            }
            if let Some(confirmation) = state.after_reload.take() {
                state.notice = Some(confirmation);
            }
            if state.entries().is_empty() {
                state.notice = Some(Notice::Error(if state.offers.is_empty() {
                    "There are no protected areas yet, and snapall found nothing it can protect here: it works with disks formatted as Btrfs.".into()
                } else {
                    "There are no protected areas yet, so there is nothing to show. Press p to protect one.".into()
                }));
            }
            load_selected_if_needed(state)
        }
    }
}

pub(crate) fn on_snapshots_loaded(
    state: &mut State,
    area: &AreaName,
    result: Result<Vec<SnapshotInfo>, String>,
) -> Vec<Effect> {
    let selected = state.selected_area().map(|c| c.name.clone());
    let Some(entry) = state.entry_mut(area) else {
        return Vec::new();
    };
    match result {
        Ok(mut list) => {
            list.sort_by_key(|s| s.id);
            entry.snapshots = Load::Loaded(list);
        }
        Err(error) => {
            entry.snapshots = Load::Failed(error.clone());
            state.notice = Some(Notice::Error(format!("{area}: {error}")));
        }
    }
    if selected.as_ref() == Some(area) {
        let existing: BTreeSet<SnapshotId> =
            state.displayed_snapshots().iter().map(|s| s.id).collect();
        state.marked.retain(|id| existing.contains(id));
        let len = existing.len();
        state.snapshot_cursor = state.snapshot_cursor.min(len.saturating_sub(1));
    }
    Vec::new()
}

pub(crate) fn on_created(
    state: &mut State,
    results: Vec<(AreaName, Result<SnapshotId, String>)>,
) -> Vec<Effect> {
    let created: Vec<String> = results
        .iter()
        .filter_map(|(area, r)| r.as_ref().ok().map(|id| format!("{area} #{id}")))
        .collect();
    let failures: Vec<String> = results
        .iter()
        .filter_map(|(area, r)| r.as_ref().err().map(|e| format!("{area}: {e}")))
        .collect();
    state.notice = Some(if failures.is_empty() {
        Notice::Info(format!("created {}", created.join(", ")))
    } else if created.is_empty() {
        Notice::Error(failures.join("; "))
    } else {
        Notice::Error(format!(
            "created {}, but {}",
            created.join(", "),
            failures.join("; ")
        ))
    });
    // New snapshots are newest: show them.
    state.snapshot_cursor = 0;
    results
        .into_iter()
        .filter_map(|(area, r)| r.ok().map(|_| Effect::LoadSnapshots(area)))
        .collect()
}
