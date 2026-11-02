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
    /// The path is under an area's directory but in another part of the disk, which that area's
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

/// How an area stands to a path.
enum Holding {
    /// The path is in the area's own part of the disk: its snapshots hold the file.
    Really {
        length: usize,
    },
    /// The path is under the area's directory but in another part of the disk, which that area's snapshots do not
    /// include.
    OnlyUnderItsDirectory {
        length: usize,
        location: String,
    },
    NotUnder,
}

/// One rule, for finding the area of a path and for reading its history: where the area lives, whether the path is
/// under that, and whether the area's snapshots really hold it.
fn holding(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    path: &str,
) -> Result<Holding, HistoryError> {
    let location = backend.area_location(area)?;
    if !under(&location, path) {
        return Ok(Holding::NotUnder);
    }
    let length = location.trim_end_matches('/').len();
    Ok(if backend.covers(area, path)? {
        Holding::Really { length }
    } else {
        Holding::OnlyUnderItsDirectory { length, location }
    })
}

/// The area whose part of the disk holds `path`: the one with the longest matching mount point, so a file in `/home`
/// belongs to `home` and not to `root`, which also contains it.
pub fn area_covering(backend: &dyn SnapshotBackend, path: &str) -> Result<AreaName, HistoryError> {
    if !clean(path) {
        return Err(HistoryError::BadPath {
            path: path.to_owned(),
        });
    }
    let mut really = Vec::new();
    let mut only_by_directory = Vec::new();
    for area in backend.list_areas().map_err(SnapshotError::from)? {
        match holding(backend, &area, path)? {
            Holding::Really { length } => really.push((length, area)),
            Holding::OnlyUnderItsDirectory { length, location } => {
                only_by_directory.push((length, area, location));
            }
            Holding::NotUnder => {}
        }
    }
    // the longest mount point wins, and the first of two equally long ones
    if let Some((_, area)) = really
        .into_iter()
        .min_by_key(|(length, _)| std::cmp::Reverse(*length))
    {
        return Ok(area);
    }
    // failing that, the most specific one that merely has it under its directory: saying "does not exist" would be wrong
    match only_by_directory
        .into_iter()
        .min_by_key(|(length, ..)| std::cmp::Reverse(*length))
    {
        Some((_, area, location)) => Err(HistoryError::OtherLocation {
            path: path.to_owned(),
            area,
            location,
        }),
        None => Err(HistoryError::NoArea {
            path: path.to_owned(),
        }),
    }
}

/// How `path` stood in every snapshot of `area`.
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
    match holding(backend, area, path)? {
        Holding::NotUnder => Err(HistoryError::NoArea {
            path: path.to_owned(),
        }),
        Holding::OnlyUnderItsDirectory { location, .. } => Err(HistoryError::OtherLocation {
            path: path.to_owned(),
            area: area.clone(),
            location,
        }),
        Holding::Really { .. } => Ok(backend.file_versions(area, path)?),
    }
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
