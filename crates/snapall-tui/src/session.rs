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
    delete_snapshot, file_versions_of, plan_snapshots, preview_restore, protect_area,
    restore_snapshot, run_plan, snapshots_of, Point, SnapshotBackend, SnapshotError,
};

use crate::model::{update, Effect, Key, Msg, State, UiContext};

/// Performs effects. `Sync` is required of the backend so one executor can serve a worker thread.
#[derive(Clone, Copy)]
pub struct Executor<'a> {
    backend: &'a (dyn SnapshotBackend + Sync),
}

impl<'a> Executor<'a> {
    pub fn new(backend: &'a (dyn SnapshotBackend + Sync)) -> Self {
        Self { backend }
    }

    /// Perform one effect. Always answers with exactly one message.
    pub fn execute(&self, effect: Effect) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        match effect {
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
                    Ok(plan) => {
                        run_plan(backend, &plan, &description, Some(&Point::at(unix_now())))
                            .outcomes
                            .into_iter()
                            .map(|o| (o.config, o.result.map_err(|e| e.to_string())))
                            .collect()
                    }
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
        let mut session = Self {
            state: State::with_context(context),
            executor: Executor::new(backend),
        };
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
