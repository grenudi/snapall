// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support: an in-memory adapter and the contract every backend must satisfy.
//! Compiled for this crate's own tests and for adapter crates via the `testing` feature.

// Test support: a failed assertion is a panic by design, and a poisoned test mutex is a bug in
// the test. The deny on `unwrap`/`expect`/`panic` is for production code, not for this.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::domain::{
    Change, ChangeKind, ConfigName, Description, FileState, FileVersion, RestoreSummary,
    SnapshotId, SnapshotInfo, SnapshotKind,
};
use crate::port::{BackendError, SnapshotBackend};

#[derive(Default)]
struct ConfigState {
    next_id: u64,
    snapshots: Vec<SnapshotInfo>,
    /// Where the config is mounted.
    subvolume: String,
    /// The live files: absolute path to content.
    files: BTreeMap<String, String>,
    /// The files as each snapshot saw them, by snapshot number.
    saved: BTreeMap<u64, BTreeMap<String, String>>,
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
                let subvolume = format!("/mnt/{}", name.as_str());
                state.configs.insert(
                    name,
                    ConfigState {
                        next_id: 1,
                        snapshots: Vec::new(),
                        subvolume,
                        files: BTreeMap::new(),
                        saved: BTreeMap::new(),
                    },
                );
            }
        }
        backend
    }

    /// Make `name` the system root: its subvolume is `/`.
    pub fn with_root_config(self, name: &str) -> Self {
        let name = ConfigName::parse(name).expect("valid test config name");
        if let Some(entry) = self.lock().configs.get_mut(&name) {
            entry.subvolume = "/".into();
        }
        self
    }

    /// Write a live file (creating or replacing it).
    pub fn write_file(&self, config: &str, path: &str, content: &str) {
        let name = ConfigName::parse(config).expect("valid test config name");
        let mut state = self.lock();
        let entry = state.configs.get_mut(&name).expect("the config exists");
        entry.files.insert(path.to_owned(), content.to_owned());
    }

    /// Remove a live file.
    pub fn remove_file(&self, config: &str, path: &str) {
        let name = ConfigName::parse(config).expect("valid test config name");
        let mut state = self.lock();
        let entry = state.configs.get_mut(&name).expect("the config exists");
        entry.files.remove(path);
    }

    /// A live file's content, if it exists.
    pub fn read_file(&self, config: &str, path: &str) -> Option<String> {
        let name = ConfigName::parse(config).expect("valid test config name");
        self.lock().configs.get(&name)?.files.get(path).cloned()
    }

    /// From now on, `create_snapshot` for `name` fails (unlike [`Self::failing_on`], usable after
    /// the backend is already in use).
    pub fn refuse_snapshots_of(&self, name: &str) {
        let name = ConfigName::parse(name).expect("valid test config name");
        self.lock().failing.insert(name);
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
        entry.saved.insert(id.get(), entry.files.clone());
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
        entry.saved.remove(&id.get());
        if entry.snapshots.len() == before {
            return Err(BackendError::CommandFailed {
                program: "in-memory".into(),
                status: "not found".into(),
                stderr: format!("snapshot {id} of {config} not found"),
            });
        }
        Ok(())
    }

    fn config_subvolume(&self, config: &ConfigName) -> Result<String, BackendError> {
        self.lock()
            .configs
            .get(config)
            .map(|entry| entry.subvolume.clone())
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))
    }

    fn changes_since(
        &self,
        config: &ConfigName,
        id: SnapshotId,
    ) -> Result<Vec<Change>, BackendError> {
        let state = self.lock();
        let entry = state
            .configs
            .get(config)
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))?;
        let saved = entry
            .saved
            .get(&id.get())
            .ok_or_else(|| not_found(config, id))?;
        Ok(diff(saved, &entry.files))
    }

    fn file_versions(
        &self,
        config: &ConfigName,
        path: &str,
    ) -> Result<Vec<FileVersion>, BackendError> {
        let state = self.lock();
        let entry = state
            .configs
            .get(config)
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))?;
        let now = entry.files.get(path);
        Ok(entry
            .snapshots
            .iter()
            .map(|snapshot| {
                let then = entry
                    .saved
                    .get(&snapshot.id.get())
                    .and_then(|files| files.get(path));
                let file_state = match (then, now) {
                    (None, _) => FileState::AbsentThen,
                    (Some(_), None) => FileState::DeletedSince,
                    (Some(before), Some(after)) if before == after => FileState::Same,
                    (Some(_), Some(_)) => FileState::Changed,
                };
                FileVersion {
                    snapshot: snapshot.clone(),
                    state: file_state,
                }
            })
            .collect())
    }

    fn undo_changes(
        &self,
        config: &ConfigName,
        id: SnapshotId,
        path: Option<&str>,
    ) -> Result<RestoreSummary, BackendError> {
        let mut state = self.lock();
        let entry = state
            .configs
            .get_mut(config)
            .ok_or_else(|| BackendError::UnknownConfig(config.clone()))?;
        let saved = entry
            .saved
            .get(&id.get())
            .cloned()
            .ok_or_else(|| not_found(config, id))?;
        let mut summary = RestoreSummary::default();
        for change in diff(&saved, &entry.files) {
            if let Some(path) = path {
                let base = path.trim_end_matches('/');
                if !(change.path == base || change.path.starts_with(&format!("{base}/"))) {
                    continue;
                }
            }
            match change.kind {
                ChangeKind::Added => {
                    entry.files.remove(&change.path);
                    summary.deleted += 1;
                }
                ChangeKind::Removed | ChangeKind::Changed => {
                    if let Some(content) = saved.get(&change.path) {
                        entry.files.insert(change.path.clone(), content.clone());
                    }
                    if change.kind == ChangeKind::Removed {
                        summary.created += 1;
                    } else {
                        summary.modified += 1;
                    }
                }
            }
        }
        Ok(summary)
    }
}

fn not_found(config: &ConfigName, id: SnapshotId) -> BackendError {
    BackendError::CommandFailed {
        program: "in-memory".into(),
        status: "not found".into(),
        stderr: format!("snapshot {id} of {config} not found"),
    }
}

fn diff(saved: &BTreeMap<String, String>, now: &BTreeMap<String, String>) -> Vec<Change> {
    let mut changes = Vec::new();
    for (path, content) in now {
        match saved.get(path) {
            None => changes.push(Change {
                kind: ChangeKind::Added,
                path: path.clone(),
            }),
            Some(old) if old != content => {
                changes.push(Change {
                    kind: ChangeKind::Changed,
                    path: path.clone(),
                });
            }
            Some(_) => {}
        }
    }
    for path in saved.keys().filter(|path| !now.contains_key(*path)) {
        changes.push(Change {
            kind: ChangeKind::Removed,
            path: path.clone(),
        });
    }
    changes.sort_by(|a, b| a.path.cmp(&b.path));
    changes
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

    // Restoring is checked read-only here (mutation is backend-specific): where the config lives,
    // what changed since a snapshot, and that a snapshot that does not exist is an error.
    let subvolume = backend
        .config_subvolume(existing)
        .expect("config_subvolume succeeds");
    assert!(
        subvolume.starts_with('/'),
        "a mount point is absolute: {subvolume}"
    );
    backend
        .changes_since(existing, second)
        .expect("changes_since an existing snapshot succeeds");
    let nowhere = SnapshotId::new(999_999).expect("nonzero");
    assert!(
        backend.changes_since(existing, nowhere).is_err(),
        "changes since a snapshot that does not exist must be an error"
    );
    assert!(
        backend.undo_changes(existing, nowhere, None).is_err(),
        "undoing to a snapshot that does not exist must be an error"
    );
    assert!(backend.config_subvolume(&missing).is_err());
    // A file's history is read-only too: for a path that was never there, every snapshot says so.
    let never = format!(
        "{}/snapall-contract-never-existed",
        subvolume.trim_end_matches('/')
    );
    let history = backend
        .file_versions(existing, &never)
        .expect("file_versions succeeds for a path that does not exist");
    assert!(
        history.iter().all(|v| v.state == FileState::AbsentThen),
        "a file that never existed is absent in every snapshot"
    );
    assert!(backend.file_versions(&missing, &never).is_err());

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
