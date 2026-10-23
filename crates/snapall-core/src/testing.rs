//! Test support: an in-memory adapter and the contract every backend must satisfy.
//! Compiled for this crate's own tests and for adapter crates via the `testing` feature.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::domain::{ConfigName, Description, SnapshotId};
use crate::port::{BackendError, SnapshotBackend};

#[derive(Default)]
struct State {
    next_id: BTreeMap<ConfigName, u64>,
    failing: BTreeSet<ConfigName>,
    taken: Vec<(ConfigName, SnapshotId, String)>,
}

/// A backend that keeps everything in memory. Fast core tests need nothing else.
#[derive(Default)]
pub struct InMemoryBackend {
    state: Mutex<State>,
}

impl InMemoryBackend {
    pub fn with_configs(names: &[&str]) -> Self {
        let backend = Self::default();
        {
            let mut state = backend.lock();
            for name in names {
                let name = ConfigName::parse(name).expect("valid test config name");
                state.next_id.insert(name, 1);
            }
        }
        backend
    }

    /// Make every `create_snapshot` for `name` fail.
    pub fn failing_on(self, name: &str) -> Self {
        let name = ConfigName::parse(name).expect("valid test config name");
        self.lock().failing.insert(name);
        self
    }

    /// Every snapshot taken so far: (config, id, description).
    pub fn snapshots(&self) -> Vec<(ConfigName, SnapshotId, String)> {
        self.lock().taken.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("state mutex poisoned")
    }
}

impl SnapshotBackend for InMemoryBackend {
    fn list_configs(&self) -> Result<Vec<ConfigName>, BackendError> {
        Ok(self.lock().next_id.keys().cloned().collect())
    }

    fn create_snapshot(
        &self,
        config: &ConfigName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError> {
        let mut state = self.lock();
        if state.failing.contains(config) {
            return Err(BackendError::CommandFailed {
                program: "in-memory".into(),
                status: "injected failure".into(),
                stderr: format!("snapshot of {config} refused"),
            });
        }
        let next = state
            .next_id
            .get_mut(config)
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))?;
        let id = SnapshotId::new(*next);
        *next += 1;
        state
            .taken
            .push((config.clone(), id, description.as_str().to_owned()));
        Ok(id)
    }
}

/// The behaviour every `SnapshotBackend` must have, whatever sits behind it.
/// `existing` must name a config the backend can snapshot.
pub fn backend_contract(backend: &dyn SnapshotBackend, existing: &ConfigName) {
    let configs = backend.list_configs().expect("list_configs succeeds");
    assert!(
        configs.contains(existing),
        "list_configs should include `{existing}`, got {configs:?}"
    );

    let description = Description::parse("snapall contract").expect("valid description");
    let first = backend
        .create_snapshot(existing, &description)
        .expect("first snapshot succeeds");
    let second = backend
        .create_snapshot(existing, &description)
        .expect("second snapshot succeeds");
    assert!(second > first, "ids must increase: {first} then {second}");

    let missing = ConfigName::parse("snapall-no-such-config").expect("valid name");
    assert!(
        backend.create_snapshot(&missing, &description).is_err(),
        "a snapshot of a missing config must be an error"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_backend_satisfies_the_contract() {
        let backend = InMemoryBackend::with_configs(&["root"]);
        backend_contract(&backend, &ConfigName::parse("root").unwrap());
    }
}
