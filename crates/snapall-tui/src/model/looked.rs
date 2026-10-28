// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What looking brought back arriving in the window: the areas, snapshots, history, health, groups and points.

use super::*;
use crate::tabs::health::HealthData;
use snapall_core::{AreaName, FileVersion, Preview, SnapshotId, SnapshotInfo};

pub(super) fn unprotect_plan_loaded(
    state: &mut State,
    area: AreaName,
    plan: Result<snapall_core::protecting::unprotect::Plan, String>,
) -> Vec<Effect> {
    if let Some(Modal::StopProtecting(prompt)) = &mut state.modal {
        if prompt.area == area {
            prompt.plan = Some(plan);
        }
    }
    Vec::new()
}

pub(super) fn limits_data(
    state: &mut State,
    area: AreaName,
    snapshots: Vec<snapall_core::SnapshotInfo>,
) -> Vec<Effect> {
    if let Some(Modal::Limits(editor)) = &mut state.modal {
        if editor.area == area {
            editor.snapshots = Some(snapshots);
        }
    }
    Vec::new()
}

pub(super) fn groups_loaded(
    state: &mut State,
    result: Result<snapall_core::Groups, String>,
) -> Vec<Effect> {
    match result {
        Ok(groups) => {
            state.groups = groups;
            state.groups_error = None;
        }
        Err(why) => state.groups_error = Some(why),
    }
    Vec::new()
}

pub(super) fn history_loaded(
    state: &mut State,
    entries: Vec<snapall_core::JournalEntry>,
) -> Vec<Effect> {
    state.activity.merge_history(&entries);
    what_this_window_started_has_ended(state, &entries)
}

pub(super) fn health_loaded(
    state: &mut State,
    result: Result<Box<HealthData>, String>,
) -> Vec<Effect> {
    match result {
        Ok(data) => {
            state.health.data = Some(*data);
            state.health.error = None;
            state.health.keep_cursor_in_range();
        }
        Err(why) => state.health.error = Some(why),
    }
    Vec::new()
}

pub(super) fn point_offer_loaded(
    state: &mut State,
    area: AreaName,
    id: SnapshotId,
    offer: Option<PointOffer>,
) -> Vec<Effect> {
    settle(state);
    // the answer belongs to the question that asked, and to no other: if it was closed or is about another snapshot, it is dropped
    if let Some(Modal::Restore {
        area: asked,
        id: asked_id,
        offer: slot,
        ..
    }) = &mut state.modal
    {
        if *asked == area && *asked_id == id {
            *slot = offer;
        }
    }
    Vec::new()
}

pub(super) fn points_loaded(
    state: &mut State,
    result: Result<Vec<snapall_core::RestorePoint>, String>,
) -> Vec<Effect> {
    settle(state);
    on_points_loaded(state, result)
}

pub(super) fn point_preview_loaded(state: &mut State, offer: PointOffer) -> Vec<Effect> {
    settle(state);
    if let Some(Modal::Points {
        index,
        points: Some(points),
        preview,
    }) = &mut state.modal
    {
        if points
            .get(*index)
            .is_some_and(|point| point.point == offer.point.point)
        {
            *preview = Some(offer);
        }
    }
    Vec::new()
}

pub(super) fn changes_loaded(
    state: &mut State,
    area: AreaName,
    id: SnapshotId,
    result: Result<Preview, String>,
) -> Vec<Effect> {
    settle(state);
    on_changes_loaded(state, &area, id, result)
}

pub(super) fn file_versions_loaded(
    state: &mut State,
    area: AreaName,
    path: String,
    result: Result<Vec<FileVersion>, String>,
) -> Vec<Effect> {
    settle(state);
    on_file_versions_loaded(state, &area, &path, result)
}

pub(super) fn areas_loaded(
    state: &mut State,
    result: Result<Vec<AreaName>, String>,
) -> Vec<Effect> {
    settle(state);
    on_areas_loaded(state, result)
}

pub(super) fn snapshots_loaded(
    state: &mut State,
    area: AreaName,
    result: Result<Vec<SnapshotInfo>, String>,
) -> Vec<Effect> {
    settle(state);
    on_snapshots_loaded(state, &area, result)
}
