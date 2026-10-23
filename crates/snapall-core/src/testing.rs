//! Test support: an in-memory adapter and the contract every backend must satisfy.
//! Compiled for this crate's own tests and for adapter crates via the `testing` feature.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::domain::{ConfigName, Description, SnapshotId, SnapshotInfo, SnapshotKind};
use crate::port::{BackendError, SnapshotBackend};

#[derive(Default)]
struct ConfigState {
    next_id: u64,
    snapshots: Vec<SnapshotInfo>,
}

#[derive(Default)]
struct State {
    configs: BTreeMap<ConfigName, ConfigState>,
    failing: BTreeSet<ConfigName>,
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
                state.configs.insert(
                    name,
                    ConfigState {
                        next_id: 1,
                        snapshots: Vec::new(),
                    },
                );
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
        self.lock()
            .configs
            .iter()
            .flat_map(|(config, state)| {
                state
                    .snapshots
                    .iter()
                    .map(move |s| (config.clone(), s.id, s.description.clone()))
            })
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("state mutex poisoned")
    }
}

impl SnapshotBackend for InMemoryBackend {
    fn list_configs(&self) -> Result<Vec<ConfigName>, BackendError> {
        Ok(self.lock().configs.keys().cloned().collect())
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
        let entry = state
            .configs
            .get_mut(config)
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))?;
        let id = SnapshotId::new(entry.next_id).expect("ids start at 1");
        entry.next_id += 1;
        entry.snapshots.push(SnapshotInfo {
            id,
            kind: SnapshotKind::Single,
            date: Some(format!("2026-01-01 00:00:{:02}", id.get() % 60)),
            user: "root".into(),
            cleanup: Some("number".into()),
            description: description.as_str().to_owned(),
        });
        Ok(id)
    }

    fn list_snapshots(&self, config: &ConfigName) -> Result<Vec<SnapshotInfo>, BackendError> {
        self.lock()
            .configs
            .get(config)
            .map(|entry| entry.snapshots.clone())
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))
    }

    fn delete_snapshot(&self, config: &ConfigName, id: SnapshotId) -> Result<(), BackendError> {
        let mut state = self.lock();
        let entry = state
            .configs
            .get_mut(config)
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))?;
        let before = entry.snapshots.len();
        entry.snapshots.retain(|s| s.id != id);
        if entry.snapshots.len() == before {
            return Err(BackendError::CommandFailed {
                program: "in-memory".into(),
                status: "not found".into(),
                stderr: format!("snapshot {id} of {config} not found"),
            });
        }
        Ok(())
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

    // Creating
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

    // Listing: oldest first, and what we created is there, as we described it.
    let listed = backend
        .list_snapshots(existing)
        .expect("list_snapshots succeeds");
    assert!(
        listed.windows(2).all(|pair| pair[0].id < pair[1].id),
        "snapshots must be listed oldest first: {listed:?}"
    );
    let created = listed
        .iter()
        .find(|s| s.id == second)
        .unwrap_or_else(|| panic!("snapshot {second} should be listed: {listed:?}"));
    assert_eq!(created.description, "snapall contract");
    assert_eq!(created.kind, SnapshotKind::Single);
    assert!(
        backend.list_snapshots(&missing).is_err(),
        "listing a missing config must be an error"
    );

    // Deleting: gone afterwards, and deleting again is an error, not a silent success.
    backend
        .delete_snapshot(existing, first)
        .expect("deleting an existing snapshot succeeds");
    let after = backend
        .list_snapshots(existing)
        .expect("list_snapshots succeeds");
    assert!(
        !after.iter().any(|s| s.id == first),
        "snapshot {first} should be gone: {after:?}"
    );
    assert!(
        after.iter().any(|s| s.id == second),
        "other snapshots must survive a delete"
    );
    assert!(
        backend.delete_snapshot(existing, first).is_err(),
        "deleting a snapshot twice must be an error"
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
