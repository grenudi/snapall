//! The imperative shell around the pure model: runs effects against a backend and feeds
//! the results back in as messages until nothing is left to do.

use std::collections::VecDeque;

use snapall_core::{
    delete_snapshot, plan_snapshots, run_plan, snapshots_of, SnapshotBackend, SnapshotError,
};

use crate::model::{update, Effect, Key, Msg, State};

pub struct Session<'a> {
    pub state: State,
    backend: &'a dyn SnapshotBackend,
}

impl<'a> Session<'a> {
    /// Start a session and load the configs.
    pub fn new(backend: &'a dyn SnapshotBackend) -> Self {
        let mut session = Self {
            state: State::default(),
            backend,
        };
        let reply = session.execute(Effect::LoadConfigs);
        session.dispatch(reply);
        session
    }

    pub fn press(&mut self, key: Key) {
        self.dispatch(Msg::Key(key));
    }

    /// Process a message and every follow-up it causes.
    pub fn dispatch(&mut self, msg: Msg) {
        let mut queue = VecDeque::from([msg]);
        while let Some(msg) = queue.pop_front() {
            for effect in update(&mut self.state, msg) {
                queue.push_back(self.execute(effect));
            }
        }
    }

    /// Perform one effect. Always answers with exactly one message.
    fn execute(&self, effect: Effect) -> Msg {
        match effect {
            Effect::LoadConfigs => {
                Msg::ConfigsLoaded(self.backend.list_configs().map_err(|e| e.to_string()))
            }
            Effect::LoadSnapshots(config) => {
                let result = snapshots_of(self.backend, &config).map_err(|e| e.to_string());
                Msg::SnapshotsLoaded { config, result }
            }
            Effect::Create {
                targets,
                description,
            } => {
                let planned = plan_snapshots(self.backend, Some(&targets));
                Msg::Created(match planned {
                    Ok(plan) => run_plan(self.backend, &plan, &description)
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
            Effect::Delete { config, ids } => {
                let results = ids
                    .into_iter()
                    .map(|id| {
                        let result =
                            delete_snapshot(self.backend, &config, id).map_err(|e| describe(&e));
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
