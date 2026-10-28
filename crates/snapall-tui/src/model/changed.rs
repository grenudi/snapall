// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a finished change leaves behind in the window: its result, what to show of it, and what to look at again.

use super::*;
use crate::tabs::activity::Activity;
use snapall_core::{AreaName, Restored, SnapshotId};

pub(super) fn job_controlled(
    state: &mut State,
    unit: String,
    action: snapall_core::JobAction,
    result: Result<(), String>,
) -> Vec<Effect> {
    settle(state);
    state.notice = Some(match result {
        Ok(()) => Notice::Info(format!(
            "{}: {} done",
            job_title(&unit),
            action.label().to_lowercase()
        )),
        Err(why) => Notice::Error(format!("Could not do that to {}: {why}", job_title(&unit))),
    });
    Vec::new()
}

pub(super) fn quotas_set(
    state: &mut State,
    turn_on: bool,
    result: Result<(), String>,
) -> Vec<Effect> {
    settle(state);
    state.notice = Some(match (turn_on, result) {
                (true, Ok(())) => Notice::Info("Counting is on: the disk is building its tally in the background, and figures appear as it finishes".into()),
                (false, Ok(())) => Notice::Info("Counting is off".into()),
                (_, Err(why)) => Notice::Error(format!("Could not change how room is counted: {why}")),
            });
    Vec::new()
}

pub(super) fn protection_stopped(
    state: &mut State,
    area: AreaName,
    result: Result<usize, String>,
) -> Vec<Effect> {
    settle(state);
    match result {
        Ok(deleted) => {
            let told = Notice::Info(format!(
                        "{area} is no longer protected: {deleted} snapshot{} deleted, its files are untouched",
                        if deleted == 1 { "" } else { "s" }
                    ));
            // looking at the lists again would clear it: it is shown again when they have arrived
            state.after_reload = Some(told.clone());
            state.notice = Some(told);
            // the areas listed have changed
            vec![Effect::LoadAreas]
        }
        Err(why) => {
            state.notice = Some(Notice::Error(format!(
                "Could not stop protecting {area}: {why}"
            )));
            Vec::new()
        }
    }
}

pub(super) fn limits_set(
    state: &mut State,
    area: AreaName,
    result: Result<(), String>,
) -> Vec<Effect> {
    settle(state);
    state.notice = Some(match result {
        Ok(()) => Notice::Info(format!(
            "{area} now keeps what you set. It takes effect at the next cleanup."
        )),
        Err(why) => Notice::Error(format!("Could not change the limits of {area}: {why}")),
    });
    Vec::new()
}

pub(super) fn groups_saved(
    state: &mut State,
    groups: snapall_core::Groups,
    result: Result<(), String>,
) -> Vec<Effect> {
    settle(state);
    state.notice = Some(match result {
        Ok(()) => {
            state.groups = groups;
            state.groups_error = None;
            Notice::Info("The groups are saved.".into())
        }
        Err(why) => Notice::Error(format!("The groups could not be saved: {why}")),
    });
    Vec::new()
}

pub(super) fn started_in_background(state: &mut State, id: String, present: String) -> Vec<Effect> {
    settle(state);
    state.watching.push(Watched { id, unseen: 0 });
    state.poll_ticks = 0;
    state.notice = Some(Notice::Info(format!(
        "{present}: started in the background. It goes on if you close the window."
    )));
    vec![Effect::LoadHistory]
}

pub(super) fn stop_asked(state: &mut State, result: Result<(), String>) -> Vec<Effect> {
    settle(state);
    state.notice = Some(match result {
                Ok(()) => Notice::Info(
                    "Asked it to stop: it finishes the area it is on, then stops. Nothing is left half-done."
                        .into(),
                ),
                Err(why) => Notice::Error(format!("Could not ask it to stop: {why}")),
            });
    vec![Effect::LoadHistory]
}

pub(super) fn cleanup_switched(state: &mut State, result: Result<(), String>) -> Vec<Effect> {
    settle(state);
    match result {
        Ok(()) => {
            state.notice = Some(Notice::Info("Automatic cleanup is on.".into()));
            vec![Effect::LoadHealth]
        }
        Err(why) => {
            state.notice = Some(Notice::Error(format!(
                "Could not switch automatic cleanup on: {why}"
            )));
            Vec::new()
        }
    }
}

pub(super) fn finished(state: &mut State, done: Box<Finished>) -> Vec<Effect> {
    let Finished {
        id,
        what,
        present,
        outcome,
        took,
        at,
        way_back,
        msg,
    } = *done;
    // it is no longer being done
    if let Some(at) = state.doing.iter().position(|doing| *doing == present) {
        state.doing.remove(at);
    }
    let (status, detail) = match outcome {
        Ok(done) => (snapall_core::JournalStatus::Done, done),
        Err(why) => (snapall_core::JournalStatus::Failed, why),
    };
    state.activity.record(Activity {
        id: Some(id),
        at,
        what,
        status,
        detail,
        took: Some(took),
        source: Some(snapall_core::Source::Window),
        pid: None,
        progress: None,
        way_back,
    });
    // what changed may change how protection stands: look again
    let mut effects = handle(state, msg);
    effects.push(Effect::LoadHealth);
    effects.push(Effect::LoadHistory);
    effects
}

pub(super) fn point_restored(
    state: &mut State,
    point: String,
    result: Result<PointOutcome, String>,
) -> Vec<Effect> {
    settle(state);
    on_point_restored(state, &point, result)
}

pub(super) fn protected(
    state: &mut State,
    mount_point: String,
    result: Result<AreaName, String>,
) -> Vec<Effect> {
    settle(state);
    on_protected(state, &mount_point, result)
}

pub(super) fn restored(
    state: &mut State,
    area: AreaName,
    id: SnapshotId,
    path: Option<String>,
    result: Result<Restored, String>,
) -> Vec<Effect> {
    settle(state);
    on_restored(state, &area, id, path, result)
}

pub(super) fn created(
    state: &mut State,
    results: Vec<(AreaName, Result<SnapshotId, String>)>,
) -> Vec<Effect> {
    settle(state);
    on_created(state, results)
}

pub(super) fn deleted(
    state: &mut State,
    area: AreaName,
    results: Vec<(SnapshotId, Result<(), String>)>,
) -> Vec<Effect> {
    settle(state);
    on_deleted(state, &area, results)
}
