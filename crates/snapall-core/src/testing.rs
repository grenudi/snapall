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
    AreaName, AreaOption, Change, ChangeKind, Description, FileState, FileVersion, Point,
    RestoreSummary, Retention, SnapshotId, SnapshotInfo, SnapshotKind,
};
use crate::port::{BackendError, SnapshotBackend};

#[derive(Default)]
struct AreaState {
    next_id: u64,
    snapshots: Vec<SnapshotInfo>,
    /// Where the config is mounted.
    location: String,
    /// The live files: absolute path to content.
    files: BTreeMap<String, String>,
    /// The files as each snapshot saw them, by snapshot number.
    saved: BTreeMap<u64, BTreeMap<String, String>>,
    /// Whether snapshots are taken on a schedule, as snapper does for a new config.
    timeline_create: bool,
    keep: u32,
    min_age: u64,
    cleanup: bool,
}

#[derive(Default)]
struct State {
    areas: BTreeMap<AreaName, AreaState>,
    failing: BTreeSet<AreaName>,
    /// Directories that are the root of a separate subvolume: not part of any snapshot of a config
    /// above them, as a separate `/home` is not part of a snapshot of `/`.
    separate: Vec<String>,
    /// Operations made to fail, to test what is undone when one does.
    refuse_option_changes: bool,
    refuse_removals: bool,
}

/// A backend that keeps everything in memory. Fast core tests need nothing else.
#[derive(Default)]
pub struct InMemoryBackend {
    state: Mutex<State>,
}

impl InMemoryBackend {
    pub fn with_areas(names: &[&str]) -> Self {
        let backend = Self::default();
        {
            let mut state = backend.lock();
            for name in names {
                let name = AreaName::parse(name).expect("valid test config name");
                let location = format!("/mnt/{}", name.as_str());
                state.areas.insert(
                    name,
                    AreaState {
                        next_id: 1,
                        snapshots: Vec::new(),
                        location,
                        files: BTreeMap::new(),
                        saved: BTreeMap::new(),
                        timeline_create: true,
                        keep: 50,
                        min_age: 1800,
                        cleanup: true,
                    },
                );
            }
        }
        backend
    }

    /// Make `name` the system root: its subvolume is `/`.
    pub fn with_root_area(self, name: &str) -> Self {
        let name = AreaName::parse(name).expect("valid test config name");
        if let Some(entry) = self.lock().areas.get_mut(&name) {
            entry.location = "/".into();
        }
        self
    }

    fn create(
        &self,
        area: &AreaName,
        description: &Description,
        point: Option<Point>,
    ) -> Result<SnapshotId, BackendError> {
        let mut state = self.lock();
        if state.failing.contains(area) {
            return Err(BackendError::CommandFailed {
                program: "in-memory".into(),
                status: "injected failure".into(),
                stderr: format!("snapshot of {area} refused"),
            });
        }
        let entry = state
            .areas
            .get_mut(area)
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))?;
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
            point,
            used_bytes: None,
        });
        Ok(id)
    }

    /// From now on, changing a config's settings fails.
    pub fn refuse_option_changes(&self) {
        self.lock().refuse_option_changes = true;
    }

    /// From now on, removing a config fails.
    pub fn refuse_removals(&self) {
        self.lock().refuse_removals = true;
    }

    /// Whether the config takes snapshots on a schedule, if it exists.
    pub fn timeline_create(&self, area: &str) -> Option<bool> {
        let name = AreaName::parse(area).ok()?;
        self.lock().areas.get(&name).map(|c| c.timeline_create)
    }

    /// Declare `directory` the root of a separate subvolume: everything under it is no longer
    /// covered by a config whose subvolume is above it.
    pub fn with_separate_location(self, directory: &str) -> Self {
        self.lock()
            .separate
            .push(directory.trim_end_matches('/').to_owned());
        self
    }

    /// Write a live file (creating or replacing it).
    pub fn write_file(&self, area: &str, path: &str, content: &str) {
        let name = AreaName::parse(area).expect("valid test config name");
        let mut state = self.lock();
        let entry = state.areas.get_mut(&name).expect("the config exists");
        entry.files.insert(path.to_owned(), content.to_owned());
    }

    /// Remove a live file.
    pub fn remove_file(&self, area: &str, path: &str) {
        let name = AreaName::parse(area).expect("valid test config name");
        let mut state = self.lock();
        let entry = state.areas.get_mut(&name).expect("the config exists");
        entry.files.remove(path);
    }

    /// A live file's content, if it exists.
    pub fn read_file(&self, area: &str, path: &str) -> Option<String> {
        let name = AreaName::parse(area).expect("valid test config name");
        self.lock().areas.get(&name)?.files.get(path).cloned()
    }

    /// From now on, `create_snapshot` for `name` fails (unlike [`Self::failing_on`], usable after
    /// the backend is already in use).
    pub fn refuse_snapshots_of(&self, name: &str) {
        let name = AreaName::parse(name).expect("valid test config name");
        self.lock().failing.insert(name);
    }

    /// Make every `create_snapshot` for `name` fail.
    pub fn failing_on(self, name: &str) -> Self {
        let name = AreaName::parse(name).expect("valid test config name");
        self.lock().failing.insert(name);
        self
    }

    /// Every snapshot taken so far: (config, id, description).
    pub fn snapshots(&self) -> Vec<(AreaName, SnapshotId, String)> {
        self.lock()
            .areas
            .iter()
            .flat_map(|(area, state)| {
                state
                    .snapshots
                    .iter()
                    .map(move |s| (area.clone(), s.id, s.description.clone()))
            })
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("state mutex poisoned")
    }
}

impl SnapshotBackend for InMemoryBackend {
    fn list_areas(&self) -> Result<Vec<AreaName>, BackendError> {
        Ok(self.lock().areas.keys().cloned().collect())
    }

    fn create_snapshot_in(
        &self,
        area: &AreaName,
        description: &Description,
        point: &Point,
    ) -> Result<SnapshotId, BackendError> {
        self.create(area, description, Some(point.clone()))
    }

    fn create_snapshot(
        &self,
        area: &AreaName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError> {
        self.create(area, description, None)
    }

    fn list_snapshots(&self, area: &AreaName) -> Result<Vec<SnapshotInfo>, BackendError> {
        self.lock()
            .areas
            .get(area)
            .map(|entry| entry.snapshots.clone())
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))
    }

    fn delete_snapshot(&self, area: &AreaName, id: SnapshotId) -> Result<(), BackendError> {
        let mut state = self.lock();
        let entry = state
            .areas
            .get_mut(area)
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))?;
        let before = entry.snapshots.len();
        entry.snapshots.retain(|s| s.id != id);
        entry.saved.remove(&id.get());
        if entry.snapshots.len() == before {
            return Err(BackendError::CommandFailed {
                program: "in-memory".into(),
                status: "not found".into(),
                stderr: format!("snapshot {id} of {area} not found"),
            });
        }
        Ok(())
    }

    fn area_location(&self, area: &AreaName) -> Result<String, BackendError> {
        self.lock()
            .areas
            .get(area)
            .map(|entry| entry.location.clone())
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))
    }

    fn create_area(&self, name: &AreaName, location: &str) -> Result<(), BackendError> {
        let mut state = self.lock();
        if state.areas.contains_key(name) {
            return Err(BackendError::NotSupported(format!(
                "config `{name}` exists"
            )));
        }
        state.areas.insert(
            name.clone(),
            AreaState {
                next_id: 1,
                snapshots: Vec::new(),
                location: location.to_owned(),
                files: BTreeMap::new(),
                saved: BTreeMap::new(),
                timeline_create: true,
                keep: 50,
                min_age: 1800,
                cleanup: true,
            },
        );
        Ok(())
    }

    fn delete_area(&self, name: &AreaName) -> Result<(), BackendError> {
        let mut state = self.lock();
        if state.refuse_removals {
            return Err(BackendError::NotSupported("removal refused".into()));
        }
        state
            .areas
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| BackendError::UnknownArea(name.clone()))
    }

    fn set_area_option(&self, name: &AreaName, option: AreaOption) -> Result<(), BackendError> {
        let mut state = self.lock();
        if state.refuse_option_changes {
            return Err(BackendError::NotSupported("setting change refused".into()));
        }
        let entry = state
            .areas
            .get_mut(name)
            .ok_or_else(|| BackendError::UnknownArea(name.clone()))?;
        match option {
            AreaOption::TimelineCreate(on) => entry.timeline_create = on,
            AreaOption::KeepNewest(count) => entry.keep = count,
            AreaOption::NeverRemoveYoungerThan(seconds) => entry.min_age = seconds,
            AreaOption::CleanupOfSnapshots(on) => entry.cleanup = on,
        }
        Ok(())
    }

    fn retention(&self, area: &AreaName) -> Result<Retention, BackendError> {
        let state = self.lock();
        let entry = state
            .areas
            .get(area)
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))?;
        // what a new snapper config starts with
        Ok(Retention {
            keep: Some(entry.keep),
            keep_important: Some(10),
            min_age_seconds: Some(entry.min_age),
            cleanup: entry.cleanup,
            on_a_schedule: entry.timeline_create,
            space_limit_permille: Some(500),
            quotas: false,
        })
    }

    fn covers(&self, area: &AreaName, path: &str) -> Result<bool, BackendError> {
        let state = self.lock();
        let entry = state
            .areas
            .get(area)
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))?;
        let base = entry.location.trim_end_matches('/');
        let inside = base.is_empty() || path == base || path.starts_with(&format!("{base}/"));
        // a separate subvolume below the config's own directory takes the path out of its snapshots
        let cut_off = state.separate.iter().any(|root| {
            root.len() > base.len() && (path == root || path.starts_with(&format!("{root}/")))
        });
        Ok(inside && !cut_off)
    }

    fn changes_since(&self, area: &AreaName, id: SnapshotId) -> Result<Vec<Change>, BackendError> {
        let state = self.lock();
        let entry = state
            .areas
            .get(area)
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))?;
        let saved = entry
            .saved
            .get(&id.get())
            .ok_or_else(|| not_found(area, id))?;
        Ok(diff(saved, &entry.files))
    }

    fn file_versions(&self, area: &AreaName, path: &str) -> Result<Vec<FileVersion>, BackendError> {
        let state = self.lock();
        let entry = state
            .areas
            .get(area)
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))?;
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
        area: &AreaName,
        id: SnapshotId,
        path: Option<&str>,
    ) -> Result<RestoreSummary, BackendError> {
        let mut state = self.lock();
        let entry = state
            .areas
            .get_mut(area)
            .ok_or_else(|| BackendError::UnknownArea(area.clone()))?;
        let saved = entry
            .saved
            .get(&id.get())
            .cloned()
            .ok_or_else(|| not_found(area, id))?;
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

fn not_found(area: &AreaName, id: SnapshotId) -> BackendError {
    BackendError::CommandFailed {
        program: "in-memory".into(),
        status: "not found".into(),
        stderr: format!("snapshot {id} of {area} not found"),
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
pub fn backend_contract(backend: &dyn SnapshotBackend, existing: &AreaName) {
    let areas = backend.list_areas().expect("list_configs succeeds");
    assert!(
        areas.contains(existing),
        "list_configs should include `{existing}`, got {areas:?}"
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

    let missing = AreaName::parse("snapall-no-such-config").expect("valid name");
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
    let location = backend
        .area_location(existing)
        .expect("config_subvolume succeeds");
    assert!(
        location.starts_with('/'),
        "a mount point is absolute: {location}"
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
    assert!(backend.area_location(&missing).is_err());
    assert!(
        backend
            .covers(existing, &location)
            .expect("covers succeeds"),
        "a config covers its own subvolume"
    );
    // A file's history is read-only too: for a path that was never there, every snapshot says so.
    let never = format!(
        "{}/snapall-contract-never-existed",
        location.trim_end_matches('/')
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
        let backend = InMemoryBackend::with_areas(&["root"]);
        backend_contract(&backend, &AreaName::parse("root").unwrap());
    }
}

/// A machine whose answers are set by the test: how full its disk is, which jobs run, what time it is,
/// how room is counted, who else is making a change.
#[derive(Default)]
pub struct FakeSystem {
    disk: Option<crate::system::DiskUsage>,
    services: std::sync::Mutex<BTreeMap<String, crate::system::ServiceState>>,
    details: BTreeMap<String, crate::system::UnitDetails>,
    time: Option<String>,
    files: Vec<String>,
    refuse_jobs: bool,
    actions: std::sync::Mutex<Vec<(String, crate::services::JobAction)>>,
    quotas: std::sync::Mutex<Option<crate::system::QuotaMode>>,
    busy: Option<crate::system::Busy>,
    journal: std::sync::Mutex<Vec<crate::journal::Event>>,
    background: bool,
    groups: std::sync::Mutex<crate::groups::Groups>,
    started: std::sync::Mutex<Vec<(String, Vec<String>)>>,
    stops: std::sync::Mutex<Vec<String>>,
}

impl FakeSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every disk is this full.
    pub fn with_disk(mut self, size: u64, used: u64) -> Self {
        self.disk = Some(crate::system::DiskUsage { size, used });
        self
    }

    pub fn with_service(self, unit: &str, enabled: &str, active: &str) -> Self {
        self.set_service(unit, enabled, active);
        self
    }

    fn set_service(&self, unit: &str, enabled: &str, active: &str) {
        if let Ok(mut services) = self.services.lock() {
            services.insert(
                unit.to_owned(),
                crate::system::ServiceState {
                    enabled: Some(enabled.to_owned()),
                    active: Some(active.to_owned()),
                },
            );
        }
    }

    /// What the unit file of `unit` says about it.
    pub fn with_unit_file(mut self, unit: &str, description: &str, timer: &[(&str, &str)]) -> Self {
        self.details.insert(
            unit.to_owned(),
            crate::system::UnitDetails {
                file: Some(format!("/usr/lib/systemd/system/{unit}")),
                description: Some(description.to_owned()),
                timer: timer
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            },
        );
        self
    }

    pub fn at(mut self, time: &str) -> Self {
        self.time = Some(time.to_owned());
        self
    }

    /// Starting a background job is refused, as it is without administrator rights.
    pub fn refusing_jobs(mut self) -> Self {
        self.refuse_jobs = true;
        self
    }

    /// How the disk counts room (default: not at all).
    pub fn with_quotas(self, mode: crate::system::QuotaMode) -> Self {
        if let Ok(mut quotas) = self.quotas.lock() {
            *quotas = Some(mode);
        }
        self
    }

    /// Another snapall is making a change.
    pub fn busy_with(mut self, doing: &str) -> Self {
        self.busy = Some(crate::system::Busy {
            pid: 4242,
            doing: doing.to_owned(),
            since: Some("12:03:00".to_owned()),
        });
        self
    }

    /// Groups the person already made.
    pub fn with_groups(self, groups: crate::groups::Groups) -> Self {
        if let Ok(mut held) = self.groups.lock() {
            *held = groups;
        }
        self
    }

    /// This machine can hand a change to a process that goes on without the window.
    pub fn running_in_the_background(mut self) -> Self {
        self.background = true;
        self
    }

    /// What was handed to the background, in order: the change's name and the arguments.
    pub fn handed_to_the_background(&self) -> Vec<(String, Vec<String>)> {
        self.started.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// The changes that were asked to stop, in order.
    pub fn stops_asked(&self) -> Vec<String> {
        self.stops.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// An event already in the journal, as if an earlier snapall had written it. A process id of 0 is a
    /// process that is gone.
    pub fn with_event(self, event: crate::journal::Event) -> Self {
        if let Ok(mut journal) = self.journal.lock() {
            journal.push(event);
        }
        self
    }

    /// Everything recorded so far, in order.
    pub fn events(&self) -> Vec<crate::journal::Event> {
        self.journal.lock().map(|j| j.clone()).unwrap_or_default()
    }

    /// Everything done to jobs, in order.
    pub fn job_actions(&self) -> Vec<(String, crate::services::JobAction)> {
        self.actions.lock().map(|a| a.clone()).unwrap_or_default()
    }

    /// The jobs that were switched on, in order.
    pub fn jobs_switched_on(&self) -> Vec<String> {
        self.job_actions()
            .into_iter()
            .filter(|(_, action)| *action == crate::services::JobAction::EnableNow)
            .map(|(unit, _)| unit)
            .collect()
    }

    pub fn with_file(mut self, path: &str) -> Self {
        self.files.push(path.to_owned());
        self
    }
}

impl crate::system::SystemProbe for FakeSystem {
    fn disk_usage(&self, _path: &str) -> Option<crate::system::DiskUsage> {
        self.disk
    }

    fn service(&self, unit: &str) -> crate::system::ServiceState {
        let known = self
            .services
            .lock()
            .ok()
            .and_then(|services| services.get(unit).cloned());
        known.unwrap_or(crate::system::ServiceState {
            enabled: Some("not-found".to_owned()),
            active: Some("inactive".to_owned()),
        })
    }

    fn local_time(&self) -> Option<String> {
        self.time.clone()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.files.iter().any(|f| f == path)
    }

    fn unit_details(&self, unit: &str) -> Option<crate::system::UnitDetails> {
        self.details.get(unit).cloned()
    }

    fn control_job(&self, unit: &str, action: crate::services::JobAction) -> Result<(), String> {
        use crate::services::JobAction::{DisableNow, EnableNow, Restart, Start, Stop};
        if self.refuse_jobs {
            return Err("Access denied".to_owned());
        }
        if let Ok(mut actions) = self.actions.lock() {
            actions.push((unit.to_owned(), action));
        }
        // as the real thing: each action leaves the job in the state it names
        let current = self.service(unit);
        let enabled = current
            .enabled
            .clone()
            .unwrap_or_else(|| "disabled".to_owned());
        let (enabled, active) = match action {
            EnableNow => ("enabled".to_owned(), "active"),
            DisableNow => ("disabled".to_owned(), "inactive"),
            Start | Restart => (enabled, "active"),
            Stop => (enabled, "inactive"),
        };
        self.set_service(unit, &enabled, active);
        Ok(())
    }

    fn quota_mode(&self, _path: &str) -> crate::system::QuotaMode {
        self.quotas
            .lock()
            .ok()
            .and_then(|mode| *mode)
            .unwrap_or(crate::system::QuotaMode::Off)
    }

    fn set_quotas(&self, _path: &str, on: bool) -> Result<(), String> {
        if self.refuse_jobs {
            return Err("Access denied".to_owned());
        }
        if let Ok(mut quotas) = self.quotas.lock() {
            *quotas = Some(if on {
                crate::system::QuotaMode::Full
            } else {
                crate::system::QuotaMode::Off
            });
        }
        Ok(())
    }

    fn hold_changes(&self, _doing: &str) -> Result<crate::system::Hold, crate::system::Busy> {
        match &self.busy {
            Some(busy) => Err(busy.clone()),
            None => Ok(Box::new(())),
        }
    }

    fn groups(&self) -> Result<crate::groups::Groups, String> {
        self.groups
            .lock()
            .map(|g| g.clone())
            .map_err(|e| e.to_string())
    }

    fn save_groups(&self, groups: &crate::groups::Groups) -> Result<(), String> {
        if self.refuse_jobs {
            return Err("this needs administrator rights: start snapall with sudo".to_owned());
        }
        *self.groups.lock().map_err(|e| e.to_string())? = groups.clone();
        Ok(())
    }

    fn can_run_in_background(&self) -> bool {
        self.background
    }

    fn start_in_background(&self, id: &str, arguments: &[String]) -> Result<(), String> {
        if self.refuse_jobs {
            return Err("Access denied".to_owned());
        }
        if let Ok(mut started) = self.started.lock() {
            started.push((id.to_owned(), arguments.to_vec()));
        }
        Ok(())
    }

    fn request_stop(&self, id: &str) -> Result<(), String> {
        if let Ok(mut stops) = self.stops.lock() {
            stops.push(id.to_owned());
        }
        Ok(())
    }

    fn stop_requested(&self, id: &str) -> bool {
        self.stops
            .lock()
            .is_ok_and(|stops| stops.iter().any(|stop| stop == id))
    }

    fn record(&self, event: &crate::journal::Event) {
        if let Ok(mut journal) = self.journal.lock() {
            journal.push(event.clone());
        }
    }

    fn history(&self, limit: usize) -> Vec<crate::journal::Entry> {
        let events = self.events();
        let mut entries = crate::journal::fold(&events, |pid| pid != 0);
        entries.truncate(limit);
        entries
    }
}
