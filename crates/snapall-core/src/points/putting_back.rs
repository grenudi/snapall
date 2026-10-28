// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting a restore point back: what it would change in each area, and doing it.

use super::*;
use crate::course::{Course, Unattended};
use crate::domain::{AreaName, Description, Point, SnapshotId};
use crate::port::SnapshotBackend;
use crate::usecase::{preview_restore, restore_snapshot_with, Preview, Restored, Safety};

/// What restoring one area of a point would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberView {
    /// These paths would change.
    Changes(Preview),
    /// The system itself: not put back while it runs.
    SystemRoot,
    /// It could not be looked at.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberPreview {
    pub area: AreaName,
    pub id: SnapshotId,
    pub view: MemberView,
}

/// What restoring `point` would do to each of its areas, changing nothing.
pub fn preview_point(backend: &dyn SnapshotBackend, point: &RestorePoint) -> Vec<MemberPreview> {
    point
        .members
        .iter()
        .map(|member| {
            let view = match preview_restore(backend, &member.area, member.id, None) {
                Ok(preview) if preview.is_system_root => MemberView::SystemRoot,
                Ok(preview) => MemberView::Changes(preview),
                Err(error) => MemberView::Failed(error.to_string()),
            };
            MemberPreview {
                area: member.area.clone(),
                id: member.id,
                view,
            }
        })
        .collect()
}

#[derive(Debug)]
pub enum MemberResult {
    Restored(Restored),
    /// It already matched: nothing to put back, and no safety snapshot was needed.
    AlreadyThere,
    /// The system itself: left alone.
    SystemRoot,
    Failed(String),
}

#[derive(Debug)]
pub struct MemberReport {
    pub area: AreaName,
    pub id: SnapshotId,
    pub result: MemberResult,
}

/// What restoring a point did.
#[derive(Debug)]
pub struct PointRestore {
    /// The restore point of the snapshots taken just before: restoring *it* undoes all of this.
    pub safety: Point,
    pub members: Vec<MemberReport>,
    /// It was stopped, on request, before every area had its turn: the areas in `members` are done and
    /// the rest were left as they are.
    pub stopped: bool,
}

impl PointRestore {
    pub fn succeeded(&self) -> bool {
        !self
            .members
            .iter()
            .any(|m| matches!(m.result, MemberResult::Failed(_)))
    }

    /// Whether any area was actually put back, so that there is something to undo.
    pub fn changed_anything(&self) -> bool {
        self.members
            .iter()
            .any(|m| matches!(m.result, MemberResult::Restored(_)))
    }
}

/// Put every area of `point` back to how it was, one after another. Each area takes its own safety
/// snapshot first, all tagged as the restore point `safety`. One area failing does not stop the
/// others (and every outcome is reported): they are independent, and the safety snapshots make the
/// whole thing undoable either way.
pub fn restore_point(
    backend: &dyn SnapshotBackend,
    point: &RestorePoint,
    safety: Point,
) -> PointRestore {
    restore_point_along(backend, point, safety, &Unattended)
}

/// [`restore_point`] following a [`Course`]: told before each area, and stoppable between them (never in
/// the middle of one). Stopped, the areas already put back stay put back, and `stopped` says so.
pub fn restore_point_along(
    backend: &dyn SnapshotBackend,
    point: &RestorePoint,
    safety: Point,
    course: &dyn Course,
) -> PointRestore {
    let note = Description::parse(&format!("before restoring {}", point.point)).unwrap_or_default();
    let tagging = Safety {
        point: safety.clone(),
        note,
    };
    let total = point.members.len();
    let mut members = Vec::with_capacity(total);
    let mut stopped = false;
    for (done, member) in point.members.iter().enumerate() {
        if !course.proceed(done, total, member.area.as_str()) {
            stopped = true;
            break;
        }
        let result = match preview_restore(backend, &member.area, member.id, None) {
            Err(error) => MemberResult::Failed(error.to_string()),
            Ok(preview) if preview.is_system_root => MemberResult::SystemRoot,
            Ok(preview) if preview.changes.is_empty() => MemberResult::AlreadyThere,
            Ok(_) => {
                match restore_snapshot_with(backend, &member.area, member.id, None, Some(&tagging))
                {
                    Ok(restored) => MemberResult::Restored(restored),
                    Err(error) => MemberResult::Failed(error.to_string()),
                }
            }
        };
        members.push(MemberReport {
            area: member.area.clone(),
            id: member.id,
            result,
        });
    }
    PointRestore {
        safety,
        members,
        stopped,
    }
}
