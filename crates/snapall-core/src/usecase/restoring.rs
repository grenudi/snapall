// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting an area, or one path of it, back to how a snapshot had it, with a safety snapshot first.

use super::*;
use crate::domain::{AreaName, Change, Description, Point, RestoreSummary, SnapshotId};
use crate::port::{BackendError, SnapshotBackend};
use thiserror::Error;

/// Why a restore was refused or failed.
#[derive(Debug, Error)]
pub enum RestoreError {
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("{area} has no snapshot #{id}")]
    UnknownSnapshot { area: AreaName, id: SnapshotId },
    /// Putting the whole system root back while it is running would leave files, kernel and
    /// boot menu out of step with each other. Single paths are fine; the system itself is
    /// restored from the boot menu or a live system.
    #[error("`{area}` is the system root: restoring all of it while it runs is not safe (restore single paths, or restore the system from the boot menu)")]
    SystemRoot { area: AreaName },
    #[error("`{path}` is not an absolute path without `..`")]
    BadPath { path: String },
    #[error("`{path}` is outside {area}, which is {location}")]
    OutsideArea {
        area: AreaName,
        path: String,
        location: String,
    },
    #[error("the safety snapshot could not be taken, so nothing was restored: {0}")]
    SafetySnapshot(BackendError),
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// What restoring would do, before anything is touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Where the config lives.
    pub location: String,
    /// Restoring all of it is refused (see [`RestoreError::SystemRoot`]); single paths still work.
    pub is_system_root: bool,
    /// The paths that differ now, limited to the requested path if there was one.
    pub changes: Vec<Change>,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The snapshot of the state just before, which undoes this restore.
    pub safety_snapshot: SnapshotId,
    pub summary: RestoreSummary,
}

pub(crate) fn is_root(location: &str) -> bool {
    location.trim_end_matches('/').is_empty()
}

/// A path that cannot escape: absolute, no `..` component, no control line breaks.
pub(super) fn clean(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains(['\0', '\n', '\r'])
        && !path.split('/').any(|part| part == "..")
}

/// Whether `path` is `base` or lies under it.
pub(super) fn under(base: &str, path: &str) -> bool {
    let base = base.trim_end_matches('/');
    base.is_empty()
        || path == base
        || path
            .strip_prefix(base)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub(super) fn checked(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<String, RestoreError> {
    ensure_area(backend, area)?;
    if !backend.list_snapshots(area)?.iter().any(|s| s.id == id) {
        return Err(RestoreError::UnknownSnapshot {
            area: area.clone(),
            id,
        });
    }
    let location = backend.area_location(area)?;
    if let Some(path) = path {
        if !clean(path) {
            return Err(RestoreError::BadPath {
                path: path.to_owned(),
            });
        }
        if !under(&location, path) {
            return Err(RestoreError::OutsideArea {
                area: area.clone(),
                path: path.to_owned(),
                location,
            });
        }
    }
    Ok(location)
}

/// What restoring snapshot `id` of `config` would change. Read-only.
pub fn preview_restore(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<Preview, RestoreError> {
    let location = checked(backend, area, id, path)?;
    let mut changes = backend.changes_since(area, id)?;
    if let Some(path) = path {
        changes.retain(|change| under(path, &change.path));
    }
    Ok(Preview {
        is_system_root: is_root(&location),
        location,
        changes,
    })
}

/// Put `config` (or only `path` in it) back to how snapshot `id` had it.
///
/// Two rules make this safe to offer. The state just before is snapshotted first, and if that
/// fails nothing is restored, so a restore can always be undone. And the system root is never
/// restored whole: only single paths of it.
pub fn restore_snapshot(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
) -> Result<Restored, RestoreError> {
    restore_snapshot_with(backend, area, id, path, None)
}

/// How the snapshot taken just before a restore is named and tagged, when it is one of several taken
/// by one action (restoring a whole restore point), so that undoing the action is one action too.
#[derive(Debug, Clone)]
pub struct Safety {
    pub point: Point,
    pub note: Description,
}

/// [`restore_snapshot`], with the safety snapshot tagged and named as given.
pub fn restore_snapshot_with(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
    safety: Option<&Safety>,
) -> Result<Restored, RestoreError> {
    let location = checked(backend, area, id, path)?;
    if path.is_none() && is_root(&location) {
        return Err(RestoreError::SystemRoot { area: area.clone() });
    }
    let safety_snapshot = match safety {
        Some(safety) => backend.create_snapshot_in(area, &safety.note, &safety.point),
        None => {
            let note = Description::parse(&format!("before restoring #{id}")).map_err(|_| {
                RestoreError::BadPath {
                    path: format!("#{id}"),
                }
            })?;
            backend.create_snapshot(area, &note)
        }
    }
    .map_err(RestoreError::SafetySnapshot)?;
    let summary = backend.undo_changes(area, id, path)?;
    Ok(Restored {
        safety_snapshot,
        summary,
    })
}
