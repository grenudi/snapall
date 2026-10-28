// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window does when something happens: its state, what can happen to it, and what it asks for in return.
//!
//! Nothing here touches a terminal or a backend. `update` mutates the state and returns
//! the *effects* the shell should perform; the shell reports what happened as new messages.

use std::collections::BTreeSet;
use std::time::Duration;

use crate::tab::Tab;
use crate::tabs::activity::ActivityTab;
use crate::tabs::health::{HealthData, HealthTab};
use snapall_core::{
    AreaName, Description, FileState, FileVersion, Preview, Restored, SnapshotId, SnapshotInfo,
};

mod changed;
mod file;
mod following;
mod input;
mod keys;
mod living;
mod looked;
mod messages;
mod prompts;
mod state;

pub(crate) use crate::tabs::snapshots::{on_areas_loaded, on_created, on_snapshots_loaded};
pub(crate) use file::*;
pub(crate) use following::*;
pub use input::*;
pub(crate) use keys::*;
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
pub(super) const PRESSED_TICKS: u8 = 3;

/// What a unit is called to a person: its job's title, or the unit's own name if it is not one.
pub(crate) fn job_title(unit: &str) -> String {
    snapall_core::keeping::jobs::ALL
        .iter()
        .find(|job| job.unit == unit || job.worker_unit() == unit)
        .map_or_else(|| unit.to_owned(), |job| job.title.to_owned())
}

/// What one message does. The work it starts is counted once, by [`update`], however many messages were
/// unwrapped on the way.
fn handle(state: &mut State, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::Click(click) => living::click(state, click),
        Msg::Hover(over) => living::hover(state, over),
        Msg::Key(key) => on_key(state, key),
        Msg::Refused(why) => living::refused(state, why),
        Msg::Scroll { pane, delta } => on_scroll(state, pane, delta),
        Msg::Start => living::start(state),
        Msg::Tick => living::tick(state),
        Msg::AreasLoaded(result) => looked::areas_loaded(state, result),
        Msg::ChangesLoaded { area, id, result } => looked::changes_loaded(state, area, id, result),
        Msg::FileVersionsLoaded { area, path, result } => {
            looked::file_versions_loaded(state, area, path, result)
        }
        Msg::GroupsLoaded(result) => looked::groups_loaded(state, result),
        Msg::HealthLoaded(result) => looked::health_loaded(state, result),
        Msg::HistoryLoaded(entries) => looked::history_loaded(state, entries),
        Msg::LimitsData { area, snapshots } => looked::limits_data(state, area, snapshots),
        Msg::PointOfferLoaded { area, id, offer } => {
            looked::point_offer_loaded(state, area, id, offer)
        }
        Msg::PointPreviewLoaded(offer) => looked::point_preview_loaded(state, offer),
        Msg::PointsLoaded(result) => looked::points_loaded(state, result),
        Msg::SnapshotsLoaded { area, result } => looked::snapshots_loaded(state, area, result),
        Msg::UnprotectPlanLoaded { area, plan } => looked::unprotect_plan_loaded(state, area, plan),
        Msg::CleanupSwitched(result) => changed::cleanup_switched(state, result),
        Msg::Created(results) => changed::created(state, results),
        Msg::Deleted { area, results } => changed::deleted(state, area, results),
        Msg::Finished(done) => changed::finished(state, done),
        Msg::GroupsSaved { groups, result } => changed::groups_saved(state, groups, result),
        Msg::JobControlled {
            unit,
            action,
            result,
        } => changed::job_controlled(state, unit, action, result),
        Msg::LimitsSet { area, result } => changed::limits_set(state, area, result),
        Msg::PointRestored { point, result } => changed::point_restored(state, point, result),
        Msg::Protected {
            mount_point,
            result,
        } => changed::protected(state, mount_point, result),
        Msg::ProtectionStopped { area, result } => changed::protection_stopped(state, area, result),
        Msg::QuotasSet { turn_on, result } => changed::quotas_set(state, turn_on, result),
        Msg::Restored {
            area,
            id,
            path,
            result,
        } => changed::restored(state, area, id, path, result),
        Msg::StartedInBackground { id, present } => {
            changed::started_in_background(state, id, present)
        }
        Msg::StopAsked { result, .. } => changed::stop_asked(state, result),
    }
}

/// One result came back. A result nobody asked for does not push the count below zero.
pub(super) fn settle(state: &mut State) {
    state.pending = state.pending.saturating_sub(1);
}
