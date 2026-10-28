// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Taking snapshots: planning which areas, doing it area by area, and reporting how each went.

use crate::looking::snapshots::SnapshotError;
use crate::ports::course::{Course, Unattended};
use crate::ports::snapshots::{BackendError, SnapshotBackend};
use crate::taking::plan::{plan, Plan};
use crate::vocabulary::domain::{AreaName, Description, Point, SnapshotId};

#[derive(Debug)]
pub struct Outcome {
    pub area: AreaName,
    pub result: Result<SnapshotId, BackendError>,
}

/// One outcome per planned area, in plan order.
#[derive(Debug)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
}

impl Report {
    pub fn failures(&self) -> usize {
        self.outcomes.iter().filter(|o| o.result.is_err()).count()
    }

    pub fn is_success(&self) -> bool {
        self.failures() == 0
    }
}

/// Ask the backend what exists, then decide what to do. No snapshot is taken.
pub fn plan_snapshots(
    backend: &dyn SnapshotBackend,
    only: Option<&[AreaName]>,
) -> Result<Plan, SnapshotError> {
    let available = backend.list_areas()?;
    Ok(plan(only, &available)?)
}

/// Execute a plan. One failing area does not stop the others: every area gets its
/// attempt and the caller sees every outcome.
pub fn run_plan(
    backend: &dyn SnapshotBackend,
    plan: &Plan,
    description: &Description,
    point: Option<&Point>,
) -> Report {
    run_plan_along(backend, plan, description, point, &Unattended).0
}

/// [`run_plan`], one step per area, following a [`Course`]: told before each, and stoppable between
/// them. The second answer is whether it was stopped before the end; the report then has the outcomes of
/// the steps that were taken.
pub fn run_plan_along(
    backend: &dyn SnapshotBackend,
    plan: &Plan,
    description: &Description,
    point: Option<&Point>,
    course: &dyn Course,
) -> (Report, bool) {
    let total = plan.targets().len();
    let mut outcomes = Vec::with_capacity(total);
    for (done, area) in plan.targets().iter().enumerate() {
        if !course.proceed(done, total, area.as_str()) {
            return (Report { outcomes }, true);
        }
        outcomes.push(Outcome {
            area: area.clone(),
            // every snapshot of one action carries the same restore point, so they can be found
            // together again and put back with one action
            result: match point {
                Some(point) => backend.create_snapshot_in(area, description, point),
                None => backend.create_snapshot(area, description),
            },
        });
    }
    (Report { outcomes }, false)
}

pub fn snapshot_all(
    backend: &dyn SnapshotBackend,
    only: Option<&[AreaName]>,
    description: &Description,
    point: Option<&Point>,
) -> Result<Report, SnapshotError> {
    let plan = plan_snapshots(backend, only)?;
    Ok(run_plan(backend, &plan, description, point))
}
