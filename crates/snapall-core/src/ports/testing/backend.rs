// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The in-memory stand-in doing what a real backend does.

use super::*;
use crate::ports::snapshots::{BackendError, SnapshotBackend};
use crate::vocabulary::domain::{
    AreaName, AreaOption, Change, ChangeKind, Description, FileState, FileVersion, Point,
    RestoreSummary, Retention, SnapshotId, SnapshotInfo,
};
use std::collections::BTreeMap;

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
        // a separate subvolume below the area's own directory takes the path out of its snapshots
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

pub(super) fn not_found(area: &AreaName, id: SnapshotId) -> BackendError {
    BackendError::CommandFailed {
        program: "in-memory".into(),
        status: "not found".into(),
        stderr: format!("snapshot {id} of {area} not found"),
    }
}

pub(super) fn diff(
    saved: &BTreeMap<String, String>,
    now: &BTreeMap<String, String>,
) -> Vec<Change> {
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
