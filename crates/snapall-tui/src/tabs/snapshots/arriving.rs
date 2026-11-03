// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the Snapshots tab does when the areas and their snapshots arrive, and after a snapshot is taken.

use crate::model::*;
use snapall_core::{AreaName, SnapshotId, SnapshotInfo};
use std::collections::BTreeSet;

pub(crate) fn on_areas_loaded(
    state: &mut State,
    result: Result<Vec<AreaName>, String>,
) -> Vec<Effect> {
    match result {
        Err(error) => areas_could_not_be_read(state, error),
        Ok(names) => {
            show_the_areas(state, names);
            settle_the_notice(state);
            load_selected_if_needed(state)
        }
    }
}

fn areas_could_not_be_read(state: &mut State, error: String) -> Vec<Effect> {
    state.snapshots.areas = Load::Failed(error.clone());
    state.snapshots.area_cursor = 0;
    state.waiting.after_reload = None;
    state.notice = Some(Notice::Error(error));
    Vec::new()
}

/// The areas, in order, with the cursor still on the one it was on if that one is still there. Nothing of theirs is
/// loaded yet, and nothing is marked: the old marks belonged to the old lists.
fn show_the_areas(state: &mut State, mut names: Vec<AreaName>) {
    let previous = state.snapshots.selected_area_name();
    names.sort();
    names.dedup();
    state.snapshots.area_cursor = previous
        .and_then(|p| names.iter().position(|n| *n == p))
        .unwrap_or(0);
    state.snapshots.areas = Load::Loaded(
        names
            .into_iter()
            .map(|name| AreaEntry {
                name,
                snapshots: Load::NotLoaded,
            })
            .collect(),
    );
    state.snapshots.snapshot_cursor = 0;
    state.snapshots.marked.clear();
}

/// Which notice shows now that the areas have arrived: an old piece of information goes, what a change asked to say
/// once the lists were read again shows, and with nothing protected at all the way forward is said.
fn settle_the_notice(state: &mut State) {
    if matches!(state.notice, Some(Notice::Info(_))) {
        state.notice = None;
    }
    if let Some(confirmation) = state.waiting.after_reload.take() {
        state.notice = Some(confirmation);
    }
    if state.snapshots.entries().is_empty() {
        state.notice = Some(Notice::Error(nothing_is_protected_yet(state)));
    }
}

fn nothing_is_protected_yet(state: &State) -> String {
    if state.offers.is_empty() {
        format!(
            "There are no protected areas yet, and snapall found nothing it can protect here: it works with {}.",
            snapall_core::vocabulary::explain::WORKS_WITH
        )
    } else {
        "There are no protected areas yet, so there is nothing to show. Press p to protect one."
            .to_owned()
    }
}

pub(crate) fn on_snapshots_loaded(
    state: &mut State,
    area: &AreaName,
    result: Result<Vec<SnapshotInfo>, String>,
) -> Vec<Effect> {
    let selected = state.snapshots.selected_area_name();
    let Some(entry) = state.snapshots.entry_mut(area) else {
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
        let existing: BTreeSet<SnapshotId> = state
            .snapshots
            .displayed_snapshots()
            .iter()
            .map(|s| s.id)
            .collect();
        state.snapshots.marked.retain(|id| existing.contains(id));
        let len = existing.len();
        state.snapshots.snapshot_cursor =
            state.snapshots.snapshot_cursor.min(len.saturating_sub(1));
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
    state.snapshots.snapshot_cursor = 0;
    results
        .into_iter()
        .filter_map(|(area, r)| r.ok().map(|_| Effect::LoadSnapshots(area)))
        .collect()
}
