// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The snapshots of an area: listing them, and deleting one.

use crate::ports::snapshots::{BackendError, SnapshotBackend};
use crate::taking::plan::PlanError;
use crate::vocabulary::domain::{AreaName, SnapshotId, SnapshotInfo};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error(transparent)]
    Plan(#[from] PlanError),
}

/// Fail fast, with our own message, when `config` is not one the backend manages.
pub(crate) fn ensure_area(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
) -> Result<(), SnapshotError> {
    if backend.list_areas()?.contains(area) {
        Ok(())
    } else {
        Err(PlanError::UnknownAreas(vec![area.clone()]).into())
    }
}

/// The snapshots of one area, oldest first.
pub fn snapshots_of(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
) -> Result<Vec<SnapshotInfo>, SnapshotError> {
    ensure_area(backend, area)?;
    Ok(backend.list_snapshots(area)?)
}

/// The snapshots of every area, areas sorted by name. A failure for any area is an
/// error: a partial listing would look complete.
pub fn all_snapshots(
    backend: &dyn SnapshotBackend,
) -> Result<Vec<(AreaName, Vec<SnapshotInfo>)>, SnapshotError> {
    let mut areas = backend.list_areas()?;
    areas.sort();
    areas.dedup();
    areas
        .into_iter()
        .map(|area| {
            let snapshots = backend.list_snapshots(&area)?;
            Ok((area, snapshots))
        })
        .collect()
}

/// Delete one snapshot of an existing area.
pub fn delete_snapshot(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
) -> Result<(), SnapshotError> {
    ensure_area(backend, area)?;
    Ok(backend.delete_snapshot(area, id)?)
}
