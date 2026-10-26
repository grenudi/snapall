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

use crate::model::{
    update, Effect, Finished, Key, Msg, PointOffer, PointOutcome, State, UiContext,
};
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
        let Some(what) = describe_change(&effect) else {
            return self.perform(effect);
        };
        let started = std::time::Instant::now();
        let msg = self.perform(effect);
        Msg::Finished(Box::new(Finished {
            outcome: how_it_went(&msg),
            what,
            took: started.elapsed(),
            at: self.system.local_time(),
            msg,
        }))
    }

    fn perform(&self, effect: Effect) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        match effect {
            Effect::LoadHealth => Msg::HealthLoaded(
                gather(backend, self.system)
                    .map(|facts| {
                        let report = assess(&facts);
                        Box::new(HealthData { facts, report })
                    })
                    .map_err(|error| error.to_string()),
            ),
            Effect::SwitchOnCleanup => {
                Msg::CleanupSwitched(self.system.enable_job("snapper-cleanup.timer"))
            }
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

/// What a person is told about putting a restore point back, from what happened to each area.
fn outcome_of(done: &snapall_core::PointRestore) -> PointOutcome {
    use snapall_core::MemberResult;
    let count = |wanted: fn(&MemberResult) -> bool| {
        done.members.iter().filter(|m| wanted(&m.result)).count()
    };
    PointOutcome {
        restored: count(|r| matches!(r, MemberResult::Restored(_))),
        unchanged: count(|r| matches!(r, MemberResult::AlreadyThere)),
        failed: done
            .members
            .iter()
            .filter_map(|m| match &m.result {
                MemberResult::Failed(why) => Some(format!("{}: {why}", m.config)),
                _ => None,
            })
            .collect(),
        undo: done.changed_anything().then(|| done.safety.to_string()),
    }
}

/// What an effect that changes something is called in the Activity tab; `None` for one that only looks.
fn describe_change(effect: &Effect) -> Option<String> {
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    Some(match effect {
        Effect::Create { targets, .. } => format!(
            "Took a snapshot of {}",
            targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Effect::Delete { config, ids } => {
            format!(
                "Deleted {} snapshot{} of {config}",
                ids.len(),
                plural(ids.len())
            )
        }
        Effect::Restore {
            config,
            id,
            path: Some(path),
            ..
        } => {
            format!("Restored {path} from snapshot #{id} of {config}")
        }
        Effect::Restore {
            config,
            id,
            path: None,
            ..
        } => {
            format!("Put {config} back to snapshot #{id}")
        }
        Effect::RestorePoint { point } => {
            format!("Put every area back to restore point {}", point.point)
        }
        Effect::Protect { mount_point } => {
            let title = snapall_core::coverage::describe(mount_point).title;
            format!("Protected {title} ({mount_point})")
        }
        Effect::SwitchOnCleanup => "Switched on automatic cleanup".to_owned(),
        _ => return None,
    })
}

/// How a change went, from the message that says so: what came of it, or why it failed.
fn how_it_went(msg: &Msg) -> Result<String, String> {
    let failures = |pairs: Vec<(String, &Result<String, String>)>| -> Vec<String> {
        pairs
            .into_iter()
            .filter_map(|(what, result)| result.as_ref().err().map(|why| format!("{what}: {why}")))
            .collect()
    };
    match msg {
        Msg::Created(results) => {
            let each: Vec<(String, Result<String, String>)> = results
                .iter()
                .map(|(config, result)| {
                    (
                        config.to_string(),
                        result
                            .as_ref()
                            .map(|id| format!("#{id}"))
                            .map_err(Clone::clone),
                    )
                })
                .collect();
            let failed = failures(
                each.iter()
                    .map(|(name, result)| (name.clone(), result))
                    .collect(),
            );
            if failed.is_empty() {
                Ok(format!(
                    "{} snapshot{} taken",
                    results.len(),
                    if results.len() == 1 { "" } else { "s" }
                ))
            } else {
                Err(failed.join("; "))
            }
        }
        Msg::Deleted { results, .. } => {
            let failed: Vec<String> = results
                .iter()
                .filter_map(|(id, result)| result.as_ref().err().map(|why| format!("#{id}: {why}")))
                .collect();
            if failed.is_empty() {
                Ok(String::new())
            } else {
                Err(failed.join("; "))
            }
        }
        Msg::Restored { result, .. } => result
            .as_ref()
            .map(|done| format!("How it was before is snapshot #{}", done.safety_snapshot))
            .map_err(Clone::clone),
        Msg::PointRestored {
            result: Ok(outcome),
            ..
        } if outcome.failed.is_empty() => Ok(format!(
            "{} area{} put back.{}",
            outcome.restored,
            if outcome.restored == 1 { "" } else { "s" },
            outcome.undo.as_ref().map_or(String::new(), |undo| format!(
                " To undo it: restore point {undo}"
            ))
        )),
        Msg::PointRestored {
            result: Ok(outcome),
            ..
        } => Err(outcome.failed.join("; ")),
        Msg::PointRestored {
            result: Err(why), ..
        }
        | Msg::CleanupSwitched(Err(why)) => Err(why.clone()),
        Msg::Protected { result, .. } => result
            .as_ref()
            .map(|_| "It has snapshots from now on.".to_owned())
            .map_err(Clone::clone),
        _ => Ok(String::new()),
    }
}
