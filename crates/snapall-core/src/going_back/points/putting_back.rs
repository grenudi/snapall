// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting a restore point back: what it would change in each area, and doing it.

use super::*;
use crate::going_back::restoring::{
    preview_restore, restore_snapshot_with, Preview, Restored, Safety,
};
use crate::ports::course::{Course, Unattended};
use crate::ports::snapshots::SnapshotBackend;
use crate::vocabulary::domain::{AreaName, Description, Point, SnapshotId};

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
        .map(|member| MemberPreview {
            area: member.area.clone(),
            id: member.id,
            view: look_at(backend, member),
        })
        .collect()
}

/// What restoring one area of a point would do: the one answer that both looking at a point and putting it back start from.
fn look_at(backend: &dyn SnapshotBackend, member: &Member) -> MemberView {
    match preview_restore(backend, &member.area, member.id, None) {
        Ok(preview) if preview.is_system_root => MemberView::SystemRoot,
        Ok(preview) => MemberView::Changes(preview),
        Err(error) => MemberView::Failed(error.to_string()),
    }
}

impl MemberPreview {
    /// Whether putting this area back would change anything.
    pub fn would_change(&self) -> bool {
        matches!(&self.view, MemberView::Changes(preview) if !preview.changes.is_empty())
    }
}

/// How many of the areas of a point would change if it were put back.
pub fn areas_that_would_change(previews: &[MemberPreview]) -> usize {
    previews
        .iter()
        .filter(|preview| preview.would_change())
        .count()
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
        let result = match look_at(backend, member) {
            MemberView::Failed(error) => MemberResult::Failed(error),
            MemberView::SystemRoot => MemberResult::SystemRoot,
            MemberView::Changes(preview) if preview.changes.is_empty() => {
                MemberResult::AlreadyThere
            }
            MemberView::Changes(_) => {
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
