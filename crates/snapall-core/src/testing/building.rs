// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Setting up the in-memory stand-in the way a test needs it.

use super::*;
use crate::domain::{AreaName, Description, Point, SnapshotId, SnapshotInfo, SnapshotKind};
use crate::port::BackendError;
use std::collections::BTreeMap;

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

    pub(super) fn create(
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

    pub(super) fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("state mutex poisoned")
    }
}
