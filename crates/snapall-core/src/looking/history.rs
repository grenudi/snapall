// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The history of one file: which area holds it, and what it was in each snapshot.

use crate::going_back::restoring::{clean, under};
use crate::looking::snapshots::{ensure_area, SnapshotError};
use crate::ports::snapshots::{BackendError, SnapshotBackend};
use crate::vocabulary::domain::{AreaName, FileVersion};
use thiserror::Error;

/// Why a file's history could not be read.
#[derive(Debug, Error)]
pub enum HistoryError {
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("`{path}` is not an absolute path without `..`")]
    BadPath { path: String },
    #[error("no protected area contains `{path}`")]
    NoArea { path: String },
    /// The path is under an area's directory but on another subvolume, which that area's
    /// snapshots do not include. Saying "did not exist" here would be wrong: it never could have.
    #[error("`{path}` is in a different part of the disk than {area} (`{location}`), so no snapshot of {area} contains it")]
    OtherLocation {
        path: String,
        area: AreaName,
        location: String,
    },
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// A file, the area that covers it, and how it stood in each snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHistory {
    pub area: AreaName,
    pub path: String,
    /// Oldest first, as snapper lists them.
    pub versions: Vec<FileVersion>,
}

/// The area whose subvolume holds `path`: the one with the longest matching mount point, so a
/// file in `/home` belongs to `home` and not to `root`, which also contains it.
pub fn area_covering(backend: &dyn SnapshotBackend, path: &str) -> Result<AreaName, HistoryError> {
    if !clean(path) {
        return Err(HistoryError::BadPath {
            path: path.to_owned(),
        });
    }
    // The most specific area whose subvolume really holds the file (by length of mount point),
    // and, failing that, the most specific one that merely has it under its directory.
    let mut covering: Option<(usize, AreaName)> = None;
    let mut only_by_name: Option<(usize, AreaName, String)> = None;
    for area in backend.list_areas().map_err(SnapshotError::from)? {
        let location = backend.area_location(&area)?;
        if !under(&location, path) {
            continue;
        }
        let length = location.trim_end_matches('/').len();
        if backend.covers(&area, path)? {
            if covering
                .as_ref()
                .is_none_or(|(longest, _)| length > *longest)
            {
                covering = Some((length, area));
            }
        } else if only_by_name
            .as_ref()
            .is_none_or(|(longest, ..)| length > *longest)
        {
            only_by_name = Some((length, area, location));
        }
    }
    match (covering, only_by_name) {
        (Some((_, area)), _) => Ok(area),
        (None, Some((_, area, location))) => Err(HistoryError::OtherLocation {
            path: path.to_owned(),
            area,
            location,
        }),
        (None, None) => Err(HistoryError::NoArea {
            path: path.to_owned(),
        }),
    }
}

/// How `path` stood in every snapshot of `config`.
pub fn file_versions_of(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    path: &str,
) -> Result<Vec<FileVersion>, HistoryError> {
    if !clean(path) {
        return Err(HistoryError::BadPath {
            path: path.to_owned(),
        });
    }
    ensure_area(backend, area)?;
    let location = backend.area_location(area)?;
    if !under(&location, path) {
        return Err(HistoryError::NoArea {
            path: path.to_owned(),
        });
    }
    if !backend.covers(area, path)? {
        return Err(HistoryError::OtherLocation {
            path: path.to_owned(),
            area: area.clone(),
            location,
        });
    }
    Ok(backend.file_versions(area, path)?)
}

/// The history of one file: which area covers it, and how it stood in each snapshot.
pub fn file_history(
    backend: &dyn SnapshotBackend,
    path: &str,
) -> Result<FileHistory, HistoryError> {
    let area = area_covering(backend, path)?;
    let versions = file_versions_of(backend, &area, path)?;
    Ok(FileHistory {
        area,
        path: path.to_owned(),
        versions,
    })
}
