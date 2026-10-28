// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure core of the UI: state, messages, effects and `update`.
//!
//! Nothing here touches a terminal or a backend. `update` mutates the state and returns
//! the *effects* the shell should perform; the shell reports what happened as new messages.

use std::collections::BTreeSet;
use std::time::Duration;

use crate::tab::Tab;
use crate::tabs::activity::{Activity, ActivityTab};
use crate::tabs::health::{HealthData, HealthTab};
use snapall_core::{
    AreaName, Description, FileState, FileVersion, Preview, Restored, SnapshotId, SnapshotInfo,
};

mod file;
mod following;
mod input;
mod keys;
mod lists;
mod messages;
mod prompts;
mod state;

pub(crate) use file::*;
pub(crate) use following::*;
pub use input::*;
pub(crate) use keys::*;
pub(crate) use lists::*;
pub use messages::*;
pub(crate) use prompts::*;
pub use state::*;

pub fn update(state: &mut State, msg: Msg) -> Vec<Effect> {
    // How protection stands is looked at only once what the person came to see has arrived: the worker
    // does one thing at a time, and a status refresh must never delay the first screen.
    let first_screen_arrived = matches!(
        &msg,
        Msg::SnapshotsLoaded { .. } | Msg::FileVersionsLoaded { .. } | Msg::AreasLoaded(Err(_))
    ) || matches!(&msg, Msg::AreasLoaded(Ok(areas)) if areas.is_empty());
    let mut effects = handle(state, msg);
    if first_screen_arrived && !state.health_asked {
        state.health_asked = true;
        effects.push(Effect::LoadHealth);
        effects.push(Effect::LoadHistory);
        effects.push(Effect::LoadGroups);
    }
    // looking at how protection stands is a refresh on the side: it is neither work that blocks the keys
    // that start work, nor something the screen announces
    state.pending += effects
        .iter()
        .filter(|effect| !effect.is_background())
        .count();
    state
        .doing
        .extend(effects.iter().filter_map(crate::changes::describe_now));
    if state.pending == 0 {
        state.doing.clear();
        state.busy_ticks = 0;
    }
    effects
}

/// How long a click leaves its button looking pressed: a few tenths of a second.
const PRESSED_TICKS: u8 = 3;

/// What a unit is called to a person: its job's title, or the unit's own name if it is not one.
pub(crate) fn job_title(unit: &str) -> String {
    snapall_core::services::ALL
        .iter()
        .find(|job| job.unit == unit || job.worker_unit() == unit)
        .map_or_else(|| unit.to_owned(), |job| job.title.to_owned())
}

/// What one message does. The work it starts is counted once, by [`update`], however many messages were
/// unwrapped on the way.
fn handle(state: &mut State, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::Start => match &state.file {
            Some(view) => vec![Effect::LoadFileVersions {
                area: view.target.area.clone(),
                path: view.target.path.clone(),
            }],
            None => vec![Effect::LoadAreas],
        },
        Msg::JobControlled {
            unit,
            action,
            result,
        } => {
            settle(state);
            state.notice = Some(match result {
                Ok(()) => Notice::Info(format!(
                    "{}: {} done",
                    job_title(&unit),
                    action.label().to_lowercase()
                )),
                Err(why) => {
                    Notice::Error(format!("Could not do that to {}: {why}", job_title(&unit)))
                }
            });
            Vec::new()
        }
        Msg::QuotasSet { turn_on, result } => {
            settle(state);
            state.notice = Some(match (turn_on, result) {
                (true, Ok(())) => Notice::Info("Counting is on: the disk is building its tally in the background, and figures appear as it finishes".into()),
                (false, Ok(())) => Notice::Info("Counting is off".into()),
                (_, Err(why)) => Notice::Error(format!("Could not change how room is counted: {why}")),
            });
            Vec::new()
        }
        Msg::UnprotectPlanLoaded { area, plan } => {
            if let Some(Modal::StopProtecting(prompt)) = &mut state.modal {
                if prompt.area == area {
                    prompt.plan = Some(plan);
                }
            }
            Vec::new()
        }
        Msg::ProtectionStopped { area, result } => {
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
        Msg::LimitsData { area, snapshots } => {
            if let Some(Modal::Limits(editor)) = &mut state.modal {
                if editor.area == area {
                    editor.snapshots = Some(snapshots);
                }
            }
            Vec::new()
        }
        Msg::LimitsSet { area, result } => {
            settle(state);
            state.notice = Some(match result {
                Ok(()) => Notice::Info(format!(
                    "{area} now keeps what you set. It takes effect at the next cleanup."
                )),
                Err(why) => Notice::Error(format!("Could not change the limits of {area}: {why}")),
            });
            Vec::new()
        }
        Msg::GroupsLoaded(result) => {
            match result {
                Ok(groups) => {
                    state.groups = groups;
                    state.groups_error = None;
                }
                Err(why) => state.groups_error = Some(why),
            }
            Vec::new()
        }
        Msg::GroupsSaved { groups, result } => {
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
        Msg::StartedInBackground { id, present } => {
            settle(state);
            state.watching.push(Watched { id, unseen: 0 });
            state.poll_ticks = 0;
            state.notice = Some(Notice::Info(format!(
                "{present}: started in the background. It goes on if you close the window."
            )));
            vec![Effect::LoadHistory]
        }
        Msg::StopAsked { result, .. } => {
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
        Msg::HistoryLoaded(entries) => {
            state.activity.merge_history(&entries);
            what_this_window_started_has_ended(state, &entries)
        }
        Msg::Refused(why) => {
            settle(state);
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
        Msg::HealthLoaded(result) => {
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
        Msg::CleanupSwitched(result) => {
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
        Msg::Finished(done) => {
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
        Msg::Tick => {
            state.spinner = state.spinner.wrapping_add(1);
            state.pressed = state.pressed.and_then(|(click, left)| {
                left.checked_sub(1)
                    .filter(|left| *left > 0)
                    .map(|left| (click, left))
            });
            state.busy_ticks = if state.pending > 0 {
                state.busy_ticks.saturating_add(1)
            } else {
                0
            };
            follow_what_runs(state)
        }
        Msg::Key(key) => on_key(state, key),
        Msg::PointOfferLoaded { area, id, offer } => {
            settle(state);
            let asked = matches!(
                &state.modal,
                Some(Modal::Restore { area: c, id: i, .. }) if *c == area && *i == id
            );
            if asked {
                state.point_offer = offer.map(|offer| (area, id, offer));
            }
            Vec::new()
        }
        Msg::PointsLoaded(result) => {
            settle(state);
            on_points_loaded(state, result)
        }
        Msg::PointPreviewLoaded(offer) => {
            settle(state);
            if state
                .chosen_point()
                .is_some_and(|p| p.point == offer.point.point)
            {
                state.point_preview = Some(offer);
            }
            Vec::new()
        }
        Msg::PointRestored { point, result } => {
            settle(state);
            on_point_restored(state, &point, result)
        }
        Msg::Protected {
            mount_point,
            result,
        } => {
            settle(state);
            on_protected(state, &mount_point, result)
        }
        Msg::Hover(over) => {
            state.hover = over;
            Vec::new()
        }
        Msg::Click(click) => {
            // shown pressed for a moment, whatever the click then does
            state.pressed = Some((click, PRESSED_TICKS));
            on_click(state, click)
        }
        Msg::Scroll { pane, delta } => on_scroll(state, pane, delta),
        Msg::ChangesLoaded { area, id, result } => {
            settle(state);
            on_changes_loaded(state, &area, id, result)
        }
        Msg::Restored {
            area,
            id,
            path,
            result,
        } => {
            settle(state);
            on_restored(state, &area, id, path, result)
        }
        Msg::FileVersionsLoaded { area, path, result } => {
            settle(state);
            on_file_versions_loaded(state, &area, &path, result)
        }
        Msg::AreasLoaded(result) => {
            settle(state);
            on_areas_loaded(state, result)
        }
        Msg::SnapshotsLoaded { area, result } => {
            settle(state);
            on_snapshots_loaded(state, &area, result)
        }
        Msg::Created(results) => {
            settle(state);
            on_created(state, results)
        }
        Msg::Deleted { area, results } => {
            settle(state);
            on_deleted(state, &area, results)
        }
    }
}

/// One result came back. A result nobody asked for does not push the count below zero.
fn settle(state: &mut State) {
    state.pending = state.pending.saturating_sub(1);
}
