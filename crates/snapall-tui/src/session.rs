// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The imperative shell around the pure model.
//!
//! [`Executor`] performs one effect against a backend and answers with the message that
//! describes the outcome. [`Session`] drives the model with an executor *synchronously*: every
//! effect runs at once and its result is fed back before the next key. That is what the tests
//! use, and it makes each key sequence deterministic. The real terminal (`term.rs`) uses the
//! same executor on a worker thread instead, so that a slow snapper call does not freeze the
//! screen.

use std::collections::VecDeque;

use snapall_core::{
    all_snapshots, assess, delete_snapshot, file_versions_of, fresh_point, gather, plan_snapshots,
    point_of, preview_point, preview_restore, protect_area, restore_point, restore_points,
    restore_snapshot, run_plan, snapshots_of, NoSystem, SnapshotBackend, SnapshotError,
    SystemProbe,
};

use crate::changes::{
    background_arguments, describe_change, describe_now, how_it_went, new_id, outcome_of,
};
use crate::model::{update, Effect, Finished, Key, Msg, PointOffer, State, UiContext};
use crate::tabs::health::HealthData;

/// Performs effects. `Sync` is required of the backend so one executor can serve a worker thread.
#[derive(Clone, Copy)]
pub struct Executor<'a> {
    backend: &'a (dyn SnapshotBackend + Sync),
    system: &'a (dyn SystemProbe + Sync),
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
        let msg = self.perform(effect);
        let outcome = how_it_went(&msg);
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
        let backend: &dyn SnapshotBackend = self.backend;
        match effect {
            Effect::LoadGroups => Msg::GroupsLoaded(self.system.groups()),
            Effect::SaveGroups { groups } => {
                let result = self.system.save_groups(&groups);
                Msg::GroupsSaved { groups, result }
            }
            Effect::StopChange { id } => {
                let result = self.system.request_stop(&id);
                Msg::StopAsked { id, result }
            }
            Effect::LoadHistory => Msg::HistoryLoaded(self.system.history(200)),
            Effect::LoadHealth => Msg::HealthLoaded(
                gather(backend, self.system)
                    .map(|facts| {
                        let report = assess(&facts);
                        Box::new(HealthData { facts, report })
                    })
                    .map_err(|error| error.to_string()),
            ),
            Effect::ControlJob { unit, action } => {
                let result = self.system.control_job(&unit, action);
                Msg::JobControlled {
                    unit,
                    action,
                    result,
                }
            }
            Effect::SetQuotas { turn_on, paths } => {
                // the disks may be one or several; asking again of one that is already so is harmless
                let result = paths
                    .iter()
                    .try_for_each(|path| self.system.set_quotas(path, turn_on));
                Msg::QuotasSet { turn_on, result }
            }
            Effect::SwitchOnCleanup => Msg::CleanupSwitched(self.system.control_job(
                snapall_core::services::CLEANUP.unit,
                snapall_core::JobAction::EnableNow,
            )),
            Effect::LoadConfigs => {
                Msg::ConfigsLoaded(backend.list_configs().map_err(|e| e.to_string()))
            }
            Effect::LoadSnapshots(config) => {
                let result = snapshots_of(backend, &config).map_err(|e| e.to_string());
                Msg::SnapshotsLoaded { config, result }
            }
            Effect::Create {
                targets,
                description,
            } => {
                let planned = plan_snapshots(backend, Some(&targets));
                Msg::Created(match planned {
                    // all the snapshots of this one action are one restore point
                    Ok(plan) => run_plan(
                        backend,
                        &plan,
                        &description,
                        Some(&fresh_point(backend, unix_now())),
                    )
                    .outcomes
                    .into_iter()
                    .map(|o| (o.config, o.result.map_err(|e| e.to_string())))
                    .collect(),
                    Err(error) => {
                        let message = describe(&error);
                        targets
                            .into_iter()
                            .map(|config| (config, Err(message.clone())))
                            .collect()
                    }
                })
            }
            Effect::LoadChanges { config, id, path } => {
                let result = preview_restore(backend, &config, id, path.as_deref())
                    .map_err(|e| e.to_string());
                Msg::ChangesLoaded { config, id, result }
            }
            Effect::Restore { config, id, path } => {
                let result = restore_snapshot(backend, &config, id, path.as_deref())
                    .map_err(|e| e.to_string());
                Msg::Restored {
                    config,
                    id,
                    path,
                    result,
                }
            }
            Effect::LoadPoints => Msg::PointsLoaded(
                all_snapshots(backend)
                    .map(|all| restore_points(&all))
                    .map_err(|error| describe(&error)),
            ),
            Effect::LoadPointPreview { point } => Msg::PointPreviewLoaded(PointOffer {
                views: preview_point(backend, &point),
                point,
            }),
            Effect::LoadPointOffer { config, id } => {
                // Is this snapshot one of several taken by one action? Failing to look is not an
                // error to show: the prompt simply does not offer the extra action.
                let offer = all_snapshots(backend).ok().and_then(|all| {
                    let points = restore_points(&all);
                    point_of(&points, &config, id)
                        .cloned()
                        .map(|point| PointOffer {
                            views: preview_point(backend, &point),
                            point,
                        })
                });
                Msg::PointOfferLoaded { config, id, offer }
            }
            Effect::RestorePoint { point } => {
                let name = point.point.to_string();
                let done = restore_point(backend, &point, fresh_point(backend, unix_now()));
                Msg::PointRestored {
                    point: name,
                    result: Ok(outcome_of(&done)),
                }
            }
            Effect::Protect { mount_point } => {
                let title = snapall_core::coverage::describe(&mount_point).title;
                let result = protect_area(backend, &mount_point)
                    .map_err(|error| snapall_core::explain::failure_text(&title, &error));
                Msg::Protected {
                    mount_point,
                    result,
                }
            }
            Effect::LoadFileVersions { config, path } => {
                let result = file_versions_of(backend, &config, &path).map_err(|e| e.to_string());
                Msg::FileVersionsLoaded {
                    config,
                    path,
                    result,
                }
            }
            Effect::Delete { config, ids } => {
                let results = ids
                    .into_iter()
                    .map(|id| {
                        let result =
                            delete_snapshot(backend, &config, id).map_err(|e| describe(&e));
                        (id, result)
                    })
                    .collect();
                Msg::Deleted { config, results }
            }
        }
    }
}

fn describe(error: &SnapshotError) -> String {
    error.to_string()
}

/// The model plus an executor, run synchronously. Used by the tests.
pub struct Session<'a> {
    pub state: State,
    executor: Executor<'a>,
}

impl<'a> Session<'a> {
    /// Start a session and load the configs.
    pub fn new(backend: &'a (dyn SnapshotBackend + Sync)) -> Self {
        Self::with_context(backend, UiContext::default())
    }

    /// A session for a machine the shell has already looked at.
    pub fn with_context(backend: &'a (dyn SnapshotBackend + Sync), context: UiContext) -> Self {
        Self::start(State::with_context(context), Executor::new(backend))
    }

    /// A session on a machine that can answer questions about its disk, its jobs and its clock.
    pub fn with_system(
        backend: &'a (dyn SnapshotBackend + Sync),
        system: &'a (dyn SystemProbe + Sync),
        context: UiContext,
    ) -> Self {
        Self::start(
            State::with_context(context),
            Executor::new(backend).with_system(system),
        )
    }

    fn start(state: State, executor: Executor<'a>) -> Self {
        let mut session = Self { state, executor };
        session.dispatch(Msg::Start);
        session
    }

    pub fn press(&mut self, key: Key) {
        self.dispatch(Msg::Key(key));
    }

    /// Process a message and every follow-up it causes, to completion.
    pub fn dispatch(&mut self, msg: Msg) {
        let mut queue = VecDeque::from([msg]);
        while let Some(msg) = queue.pop_front() {
            for effect in update(&mut self.state, msg) {
                queue.push_back(self.executor.execute(effect));
            }
        }
    }
}

/// Seconds since 1970, which is what names a restore point after the moment an action started.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
