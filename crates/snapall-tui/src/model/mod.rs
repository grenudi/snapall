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
    ConfigName, Description, FileState, FileVersion, Preview, Restored, SnapshotId, SnapshotInfo,
};

mod file;
mod input;
mod keys;
mod lists;
mod messages;
mod prompts;
mod state;

pub(crate) use file::*;
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
        Msg::SnapshotsLoaded { .. } | Msg::FileVersionsLoaded { .. } | Msg::ConfigsLoaded(Err(_))
    ) || matches!(&msg, Msg::ConfigsLoaded(Ok(configs)) if configs.is_empty());
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

/// While anything runs, or this window started something that has not yet been seen to end, the journal
/// is read once a second, so that progress moves and the end is noticed even if another program caused it.
fn follow_what_runs(state: &mut State) -> Vec<Effect> {
    let running = state
        .activity
        .entries
        .iter()
        .any(|entry| entry.status == snapall_core::JournalStatus::Running);
    if !running && state.watching.is_empty() {
        state.poll_ticks = 0;
        return Vec::new();
    }
    state.poll_ticks += 1;
    if state.poll_ticks < 10 {
        return Vec::new();
    }
    state.poll_ticks = 0;
    vec![Effect::LoadHistory]
}

/// The journal was read: a change this window handed to the background may have ended, and then the lists
/// are looked at again, because it changed what they show. One that never appears in the journal is let go
/// of after a while, with a word about it.
fn what_this_window_started_has_ended(
    state: &mut State,
    entries: &[snapall_core::JournalEntry],
) -> Vec<Effect> {
    use snapall_core::JournalStatus;
    const LET_GO_AFTER: u32 = 30;
    let mut effects = Vec::new();
    let mut notice = None;
    state.watching.retain_mut(|watched| {
        match entries.iter().find(|entry| entry.id == watched.id) {
            None => {
                watched.unseen += 1;
                if watched.unseen > LET_GO_AFTER {
                    notice = Some(Notice::Error(
                        "A change started in the background never reported: look in Activity, and at the snapshots, to see whether it happened"
                            .into(),
                    ));
                    return false;
                }
                true
            }
            Some(entry) if entry.status == JournalStatus::Running => true,
            Some(entry) => {
                notice = Some(match entry.status {
                    JournalStatus::Done if entry.detail.is_empty() => {
                        Notice::Info(format!("{}: done", entry.what))
                    }
                    JournalStatus::Done => {
                        Notice::Info(format!("{}: done. {}", entry.what, entry.detail))
                    }
                    JournalStatus::Cancelled => {
                        Notice::Info(format!("{}: stopped on request. {}", entry.what, entry.detail))
                    }
                    _ => Notice::Error(format!("{}: {}", entry.what, entry.detail)),
                });
                effects = vec![Effect::LoadConfigs, Effect::LoadHealth];
                false
            }
        }
    });
    if let Some(notice) = notice {
        // looking at the lists again would clear it: it is shown again when they have arrived
        if !effects.is_empty() {
            state.after_reload = Some(notice.clone());
        }
        state.notice = Some(notice);
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
                config: view.target.config.clone(),
                path: view.target.path.clone(),
            }],
            None => vec![Effect::LoadConfigs],
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
        Msg::UnprotectPlanLoaded { config, plan } => {
            if let Some(Modal::StopProtecting(prompt)) = &mut state.modal {
                if prompt.config == config {
                    prompt.plan = Some(plan);
                }
            }
            Vec::new()
        }
        Msg::ProtectionStopped { config, result } => {
            settle(state);
            match result {
                Ok(deleted) => {
                    let told = Notice::Info(format!(
                        "{config} is no longer protected: {deleted} snapshot{} deleted, its files are untouched",
                        if deleted == 1 { "" } else { "s" }
                    ));
                    // looking at the lists again would clear it: it is shown again when they have arrived
                    state.after_reload = Some(told.clone());
                    state.notice = Some(told);
                    // the areas listed have changed
                    vec![Effect::LoadConfigs]
                }
                Err(why) => {
                    state.notice = Some(Notice::Error(format!(
                        "Could not stop protecting {config}: {why}"
                    )));
                    Vec::new()
                }
            }
        }
        Msg::LimitsData { config, snapshots } => {
            if let Some(Modal::Limits(editor)) = &mut state.modal {
                if editor.config == config {
                    editor.snapshots = Some(snapshots);
                }
            }
            Vec::new()
        }
        Msg::LimitsSet { config, result } => {
            settle(state);
            state.notice = Some(match result {
                Ok(()) => Notice::Info(format!(
                    "{config} now keeps what you set. It takes effect at the next cleanup."
                )),
                Err(why) => {
                    Notice::Error(format!("Could not change the limits of {config}: {why}"))
                }
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
        Msg::PointOfferLoaded { config, id, offer } => {
            settle(state);
            let asked = matches!(
                &state.modal,
                Some(Modal::Restore { config: c, id: i, .. }) if *c == config && *i == id
            );
            if asked {
                state.point_offer = offer.map(|offer| (config, id, offer));
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
        Msg::ChangesLoaded { config, id, result } => {
            settle(state);
            on_changes_loaded(state, &config, id, result)
        }
        Msg::Restored {
            config,
            id,
            path,
            result,
        } => {
            settle(state);
            on_restored(state, &config, id, path, result)
        }
        Msg::FileVersionsLoaded {
            config,
            path,
            result,
        } => {
            settle(state);
            on_file_versions_loaded(state, &config, &path, result)
        }
        Msg::ConfigsLoaded(result) => {
            settle(state);
            on_configs_loaded(state, result)
        }
        Msg::SnapshotsLoaded { config, result } => {
            settle(state);
            on_snapshots_loaded(state, &config, result)
        }
        Msg::Created(results) => {
            settle(state);
            on_created(state, results)
        }
        Msg::Deleted { config, results } => {
            settle(state);
            on_deleted(state, &config, results)
        }
    }
}

/// One result came back. A result nobody asked for does not push the count below zero.
fn settle(state: &mut State) {
    state.pending = state.pending.saturating_sub(1);
}
