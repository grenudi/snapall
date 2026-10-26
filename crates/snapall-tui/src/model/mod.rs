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
    let effects = handle(state, msg);
    state.pending += effects.len();
    effects
}

/// What one message does. The work it starts is counted once, by [`update`], however many messages were
/// unwrapped on the way.
fn handle(state: &mut State, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::Start => match &state.file {
            Some(view) => vec![
                Effect::LoadFileVersions {
                    config: view.target.config.clone(),
                    path: view.target.path.clone(),
                },
                Effect::LoadHealth,
            ],
            None => vec![Effect::LoadConfigs, Effect::LoadHealth],
        },
        Msg::HealthLoaded(result) => {
            settle(state);
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
                what,
                outcome,
                took,
                at,
                msg,
            } = *done;
            state.activity.record(Activity {
                at,
                what,
                outcome,
                took,
            });
            // what changed may change how protection stands: look again
            let mut effects = handle(state, msg);
            effects.push(Effect::LoadHealth);
            effects
        }
        Msg::Tick => {
            state.spinner = state.spinner.wrapping_add(1);
            Vec::new()
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
        Msg::Hover(button) => {
            state.hover = button;
            Vec::new()
        }
        Msg::Click(click) => on_click(state, click),
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
