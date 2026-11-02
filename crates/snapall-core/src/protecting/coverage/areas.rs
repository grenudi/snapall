// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Which parts of a disk are protected and which are not.

use super::*;
use crate::vocabulary::domain::AreaName;

/// A part of the disk that snapshots can be kept for, and whether they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    /// Where it is mounted: `/`, `/home`.
    pub mount_point: String,
    /// The area that protects it, if one does.
    pub protected_by: Option<AreaName>,
}

impl Area {
    pub fn is_protected(&self) -> bool {
        self.protected_by.is_some()
    }
}

pub(super) fn normal(path: &str) -> &str {
    if path == "/" {
        path
    } else {
        path.trim_end_matches('/')
    }
}

/// The Btrfs areas of this machine, `/` first, with the area that protects each. `areas` pairs
/// every area with the subvolume it covers.
///
/// Snapper's own `.snapshots` directories are not areas, and a subvolume that is mounted twice is
/// one area.
pub fn areas(mounts: &[Mount], areas: &[(AreaName, String)]) -> Vec<Area> {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    let mut found: Vec<Area> = Vec::new();
    for mount in mounts.iter().filter(|m| m.fstype == "btrfs") {
        if mount.point.split('/').any(|part| part == ".snapshots") {
            continue;
        }
        if seen.contains(&(mount.source.as_str(), mount.root.as_str())) {
            continue;
        }
        seen.push((mount.source.as_str(), mount.root.as_str()));
        let protected_by = areas
            .iter()
            .find(|(_, location)| normal(location) == normal(&mount.point))
            .map(|(name, _)| name.clone());
        found.push(Area {
            mount_point: mount.point.clone(),
            protected_by,
        });
    }
    found.sort_by(|a, b| {
        (a.mount_point != "/", &a.mount_point).cmp(&(b.mount_point != "/", &b.mount_point))
    });
    found
}

/// The areas worth protecting that are not: where a mistake could not be undone.
pub fn gaps(areas: &[Area]) -> Vec<&Area> {
    areas
        .iter()
        .filter(|area| {
            !area.is_protected() && describe(&area.mount_point).worth == Worth::Recommended
        })
        .collect()
}
