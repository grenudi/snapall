// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Protecting a part of the disk: making it an area.

use crate::going_back::restoring::clean;
use crate::looking::snapshots::SnapshotError;
use crate::ports::snapshots::{BackendError, SnapshotBackend};
use crate::vocabulary::domain::{AreaName, AreaOption};
use thiserror::Error;

/// Why an area could not be protected.
#[derive(Debug, Error)]
pub enum SetupError {
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("`{path}` is not an absolute path without `..`")]
    BadPath { path: String },
    #[error("{path} is already protected (by `{area}`)")]
    AlreadyProtected { path: String, area: AreaName },
    #[error("no name could be chosen for the area at {path}")]
    NoName { path: String },
    /// It failed, and what had been started was undone: the disk is as it was.
    #[error("{0}")]
    Failed(BackendError),
    /// It failed half way and could not be undone: something was left behind, and says what.
    #[error("setting up {path} failed ({problem}) and the part that was done could not be undone ({undo}): the area is left as `{area}`")]
    LeftBehind {
        path: String,
        area: AreaName,
        // boxed: this is the rare case, and two whole errors would make every Result larger
        problem: Box<BackendError>,
        undo: Box<BackendError>,
    },
}

/// Start protecting the area mounted at `mount_point`: create what snapshots of it are kept in, and
/// turn off snapshots on a schedule, so that they are taken when the person asks and nothing piles up
/// on its own. If the second step fails the first is undone, so a half-made area never lingers.
/// Returns the name it was given.
pub fn protect_area(
    backend: &dyn SnapshotBackend,
    mount_point: &str,
) -> Result<AreaName, SetupError> {
    if !clean(mount_point) {
        return Err(SetupError::BadPath {
            path: mount_point.to_owned(),
        });
    }
    let areas = backend.list_areas().map_err(SnapshotError::from)?;
    for area in &areas {
        let location = backend.area_location(area).map_err(SnapshotError::from)?;
        if location.trim_end_matches('/') == mount_point.trim_end_matches('/') {
            return Err(SetupError::AlreadyProtected {
                path: mount_point.to_owned(),
                area: area.clone(),
            });
        }
    }
    let name = crate::protecting::coverage::suggest_name(mount_point, &areas).ok_or_else(|| {
        SetupError::NoName {
            path: mount_point.to_owned(),
        }
    })?;
    backend
        .create_area(&name, mount_point)
        .map_err(SetupError::Failed)?;
    if let Err(problem) = backend.set_area_option(&name, AreaOption::TimelineCreate(false)) {
        return match backend.delete_area(&name) {
            Ok(()) => Err(SetupError::Failed(problem)),
            Err(undo) => Err(SetupError::LeftBehind {
                path: mount_point.to_owned(),
                area: name,
                problem: Box::new(problem),
                undo: Box::new(undo),
            }),
        };
    }
    Ok(name)
}
