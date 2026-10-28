// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding restore points among snapshots: by their tag, or by matching what one action took together.

use crate::ports::snapshots::SnapshotBackend;
use crate::vocabulary::calendar::{compact_utc, seconds_from_snapshot_date};
use crate::vocabulary::domain::{AreaName, Point, SnapshotId, SnapshotInfo, SnapshotKind};
use std::collections::BTreeMap;

pub(super) const MATCH_WINDOW_SECONDS: i64 = 120;

/// A name for the restore point of an action that starts at `seconds`: the time, and if a point of
/// that name is already in use (two actions in one second) a `-2`, `-3`, ... after it. Two points
/// sharing a name would be one point, so a restore's safety snapshots could not be told from what
/// they protect. If the existing points cannot be read, the plain time is used.
pub fn fresh_point(backend: &dyn SnapshotBackend, seconds: u64) -> Point {
    let taken: std::collections::BTreeSet<String> =
        crate::looking::snapshots::all_snapshots(backend)
            .map(|all| {
                all.iter()
                    .flat_map(|(_, snapshots)| snapshots.iter())
                    .filter_map(|snapshot| snapshot.point.as_ref().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default();
    let base = compact_utc(seconds);
    std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .find(|candidate| !taken.contains(candidate))
        .and_then(|candidate| Point::parse(&candidate).ok())
        .unwrap_or_else(|| Point::at(seconds))
}

/// One area's snapshot in a restore point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub area: AreaName,
    pub id: SnapshotId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePoint {
    pub point: Point,
    /// What it was called when it was taken.
    pub description: String,
    /// When the earliest of its snapshots was taken, as snapper wrote it.
    pub date: Option<String>,
    /// One snapshot per area, in the order of the areas' names.
    pub members: Vec<Member>,
    /// `true` if found by its tag, `false` if matched by description and time.
    pub tagged: bool,
}

pub(super) type Listing<'a> = &'a [(AreaName, Vec<SnapshotInfo>)];

/// The restore points among these snapshots, newest first. A point needs snapshots in at least two
/// areas: with one there is nothing to put back together.
pub fn restore_points(all: Listing<'_>) -> Vec<RestorePoint> {
    let mut found = tagged_points(all);
    found.extend(matched_points(all));
    found.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.point.cmp(&a.point)));
    found
}

/// The restore point a snapshot belongs to, if any.
pub fn point_of<'a>(
    points: &'a [RestorePoint],
    area: &AreaName,
    id: SnapshotId,
) -> Option<&'a RestorePoint> {
    points
        .iter()
        .find(|point| point.members.iter().any(|m| m.area == *area && m.id == id))
}

pub(super) fn tagged_points(all: Listing<'_>) -> Vec<RestorePoint> {
    let mut by_tag: BTreeMap<&Point, Vec<(&AreaName, &SnapshotInfo)>> = BTreeMap::new();
    for (area, snapshots) in all {
        for snapshot in snapshots {
            if let Some(point) = &snapshot.point {
                by_tag.entry(point).or_default().push((area, snapshot));
            }
        }
    }
    by_tag
        .into_iter()
        .filter(|(_, rows)| distinct_areas(rows) >= 2)
        .map(|(point, rows)| build(point.clone(), &rows, true))
        .collect()
}

pub(super) fn matched_points(all: Listing<'_>) -> Vec<RestorePoint> {
    // Snapshots with no tag, taken one at a time, whose date can be read.
    let mut by_description: BTreeMap<&str, Vec<(i64, &AreaName, &SnapshotInfo)>> = BTreeMap::new();
    for (area, snapshots) in all {
        for snapshot in snapshots {
            let seconds = snapshot
                .date
                .as_deref()
                .and_then(seconds_from_snapshot_date);
            if let (None, SnapshotKind::Single, Some(seconds)) =
                (&snapshot.point, &snapshot.kind, seconds)
            {
                by_description
                    .entry(snapshot.description.as_str())
                    .or_default()
                    .push((seconds, area, snapshot));
            }
        }
    }
    let mut points = Vec::new();
    for rows in by_description.values_mut() {
        rows.sort_by_key(|(seconds, ..)| *seconds);
        let mut cluster: Vec<(i64, &AreaName, &SnapshotInfo)> = Vec::new();
        for row in rows
            .iter()
            .copied()
            .chain(std::iter::once((i64::MAX, rows[0].1, rows[0].2)))
        {
            let continues = cluster.first().is_some_and(|first| {
                row.0 != i64::MAX
                    && row.0 - first.0 <= MATCH_WINDOW_SECONDS
                    && cluster.iter().all(|(_, area, _)| *area != row.1)
            });
            if continues {
                cluster.push(row);
                continue;
            }
            if cluster.len() >= 2 {
                let first = cluster[0].0;
                let name = format!("matched-{}", compact_utc(u64::try_from(first).unwrap_or(0)));
                if let Ok(point) = Point::parse(&name) {
                    let rows: Vec<(&AreaName, &SnapshotInfo)> =
                        cluster.iter().map(|(_, c, s)| (*c, *s)).collect();
                    points.push(build(point, &rows, false));
                }
            }
            cluster.clear();
            if row.0 != i64::MAX {
                cluster.push(row);
            }
        }
    }
    points
}

pub(super) fn distinct_areas(rows: &[(&AreaName, &SnapshotInfo)]) -> usize {
    let mut names: Vec<&AreaName> = rows.iter().map(|(area, _)| *area).collect();
    names.sort();
    names.dedup();
    names.len()
}

pub(super) fn build(
    point: Point,
    rows: &[(&AreaName, &SnapshotInfo)],
    tagged: bool,
) -> RestorePoint {
    let mut members: Vec<Member> = rows
        .iter()
        .map(|(area, snapshot)| Member {
            area: (*area).clone(),
            id: snapshot.id,
        })
        .collect();
    members.sort_by(|a, b| a.area.cmp(&b.area));
    RestorePoint {
        point,
        description: rows
            .first()
            .map(|(_, snapshot)| snapshot.description.clone())
            .unwrap_or_default(),
        date: rows.iter().filter_map(|(_, s)| s.date.clone()).min(),
        members,
        tagged,
    }
}
