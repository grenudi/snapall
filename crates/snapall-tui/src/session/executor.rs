// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's executor: it performs each effect against the backend and the machine, and answers with the message that describes the outcome.

use crate::changes::{
    background_arguments, describe_change, describe_now, how_it_went, new_id, way_back,
};
use crate::model::{Effect, Finished, Msg};
use snapall_core::{NoSystem, SnapshotBackend, SnapshotError, SystemProbe};

/// Performs effects. `Sync` is required of the backend so one executor can serve a worker thread.
#[derive(Clone, Copy)]
pub struct Executor<'a> {
    pub(super) backend: &'a (dyn SnapshotBackend + Sync),
    pub(super) system: &'a (dyn SystemProbe + Sync),
}

/// A machine that can be asked nothing, for an executor that was not given one.
static UNKNOWN_MACHINE: NoSystem = NoSystem;

impl<'a> Executor<'a> {
    pub fn new(backend: &'a (dyn SnapshotBackend + Sync)) -> Self {
        Self {
            backend,
            system: &UNKNOWN_MACHINE,
        }
    }

    /// The machine to ask about disks, background jobs and the time.
    pub fn with_system(self, system: &'a (dyn SystemProbe + Sync)) -> Self {
        Self { system, ..self }
    }

    /// Perform one effect. Always answers with exactly one message: for a change, that message wrapped
    /// with what it was, how it went, when and for how long, which is what the Activity tab shows.
    pub fn execute(&self, effect: Effect) -> Msg {
        // A change of several areas goes to a process that carries on without the window, which is what
        // lets it be closed, and watched and stopped from anywhere. If that cannot be done, it is done here.
        if self.system.can_run_in_background() {
            if let Some(started) = self.hand_to_the_background(&effect) {
                return started;
            }
        }
        let Some(what) = describe_change(&effect) else {
            return self.perform(effect);
        };
        let present = describe_now(&effect).unwrap_or_default();
        // one change at a time across every snapall: a second one is told who has the right, and waits
        let _hold = match self.system.hold_changes(&present) {
            Ok(hold) => hold,
            Err(busy) => {
                return Msg::Finished(Box::new(Finished {
                    id: new_id(),
                    outcome: Err(busy.to_string()),
                    what,
                    present,
                    took: std::time::Duration::ZERO,
                    at: self.system.local_time(),
                    way_back: None,
                    msg: Msg::Refused(busy.to_string()),
                }));
            }
        };
        let id = new_id();
        self.system.record(&snapall_core::JournalEvent::Started {
            id: id.clone(),
            at: self.system.local_time().unwrap_or_default(),
            what: what.clone(),
            source: snapall_core::Source::Window,
            pid: std::process::id(),
        });
        let started = std::time::Instant::now();
        let asked_for = effect.clone();
        let msg = self.perform(effect);
        let outcome = how_it_went(&msg);
        // how to get back from it, recorded now, while what it was before is known
        let how_back = if outcome.is_ok() {
            way_back(&asked_for, &msg)
        } else {
            None
        };
        if let Some(rollback) = &how_back {
            self.system.record(&snapall_core::JournalEvent::Rollback {
                id: id.clone(),
                at: self.system.local_time().unwrap_or_default(),
                rollback: rollback.clone(),
            });
        }
        self.system.record(&snapall_core::JournalEvent::Finished {
            id: id.clone(),
            at: self.system.local_time().unwrap_or_default(),
            outcome: outcome.clone().into(),
            took_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        });
        Msg::Finished(Box::new(Finished {
            id,
            outcome,
            what,
            present,
            took: started.elapsed(),
            at: self.system.local_time(),
            way_back: how_back,
            msg,
        }))
    }

    /// Start the change as a command of its own and say so; `None` if it is not one for the background, or
    /// could not be started (it is then done here, as before).
    fn hand_to_the_background(&self, effect: &Effect) -> Option<Msg> {
        let arguments = background_arguments(effect)?;
        let id = new_id();
        self.system.start_in_background(&id, &arguments).ok()?;
        Some(Msg::StartedInBackground {
            id,
            present: describe_now(effect).unwrap_or_default(),
        })
    }

    fn perform(&self, effect: Effect) -> Msg {
        match effect {
            Effect::LoadAreas => self.load_areas(),
            Effect::LoadChanges { area, id, path } => self.load_changes(area, id, path),
            Effect::LoadFileVersions { area, path } => self.load_file_versions(area, path),
            Effect::LoadGroups => self.load_groups(),
            Effect::LoadHealth => self.load_health(),
            Effect::LoadHistory => self.load_history(),
            Effect::LoadLimitsData { area } => self.load_limits_data(area),
            Effect::LoadPointOffer { area, id } => self.load_point_offer(area, id),
            Effect::LoadPointPreview { point } => self.load_point_preview(point),
            Effect::LoadPoints => self.load_points(),
            Effect::LoadSnapshots(area) => self.load_snapshots(area),
            Effect::LoadUnprotectPlan { area } => self.load_unprotect_plan(area),
            Effect::ControlJob { unit, action } => self.control_job(unit, action),
            Effect::Create {
                targets,
                description,
            } => self.create(targets, description),
            Effect::Delete { area, ids } => self.delete(area, ids),
            Effect::Protect { mount_point } => self.protect(mount_point),
            Effect::Restore { area, id, path } => self.restore(area, id, path),
            Effect::RestorePoint { point } => self.restore_point(point),
            Effect::SaveGroups { groups, .. } => self.save_groups(groups),
            Effect::SetLimits { area, options, .. } => self.set_limits(area, options),
            Effect::SetQuotas { turn_on, paths } => self.set_quotas(turn_on, paths),
            Effect::StopChange { id } => self.stop_change(id),
            Effect::StopProtecting { area } => self.stop_protecting(area),
            Effect::SwitchOnCleanup => self.switch_on_cleanup(),
        }
    }
}

pub(super) fn describe(error: &SnapshotError) -> String {
    error.to_string()
}

/// Seconds since 1970, which is what names a restore point after the moment an action started.
pub(super) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
