// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window asks to be changed, done through the backend and the machine.

use super::executor::{describe, unix_now, Executor};
use crate::changes::outcome_of;
use crate::model::Msg;
use snapall_core::keeping::jobs;
use snapall_core::protecting::{coverage, unprotect};
use snapall_core::vocabulary::explain;
use snapall_core::AreaName;
use snapall_core::Description;
use snapall_core::SnapshotId;
use snapall_core::{
    delete_snapshot, fresh_point, plan_snapshots, protect_area, restore_point, restore_snapshot,
    run_plan, SnapshotBackend,
};

impl Executor<'_> {
    pub(super) fn stop_protecting(&self, area: AreaName) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        // worked out again now, not trusted from when the prompt opened
        let result = self
            .system
            .groups()
            .map_err(|why| why.to_string())
            .and_then(|groups| {
                let plan = unprotect::plan(backend, &groups, &area).map_err(|e| e.to_string())?;
                unprotect::stop_protecting(backend, &plan).map_err(|e| e.to_string())
            });
        Msg::ProtectionStopped { area, result }
    }

    pub(super) fn set_limits(&self, area: AreaName, options: Vec<snapall_core::AreaOption>) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let result = options
            .iter()
            .try_for_each(|option| backend.set_area_option(&area, *option))
            .map_err(|error| error.to_string());
        Msg::LimitsSet { area, result }
    }

    pub(super) fn save_groups(&self, groups: snapall_core::Groups) -> Msg {
        let result = self.system.save_groups(&groups);
        Msg::GroupsSaved { groups, result }
    }

    pub(super) fn stop_change(&self, id: String) -> Msg {
        let result = self.system.request_stop(&id);
        Msg::StopAsked { id, result }
    }

    pub(super) fn control_job(&self, unit: String, action: snapall_core::JobAction) -> Msg {
        let result = self.system.control_job(&unit, action);
        Msg::JobControlled {
            unit,
            action,
            result,
        }
    }

    pub(super) fn set_quotas(&self, turn_on: bool, paths: Vec<String>) -> Msg {
        // the disks may be one or several; asking again of one that is already so is harmless
        let result = paths
            .iter()
            .try_for_each(|path| self.system.set_quotas(path, turn_on));
        Msg::QuotasSet { turn_on, result }
    }

    pub(super) fn switch_on_cleanup(&self) -> Msg {
        Msg::CleanupSwitched(
            self.system
                .control_job(jobs::CLEANUP.unit, snapall_core::JobAction::EnableNow),
        )
    }

    pub(super) fn create(&self, targets: Vec<AreaName>, description: Description) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let planned = plan_snapshots(backend, Some(&targets));
        Msg::Created(match planned {
            // all the snapshots of this one action are one restore point
            Ok(plan) => run_plan(
                backend,
                &plan,
                &description,
                Some(&fresh_point(backend, unix_now())),
            )
            .outcomes
            .into_iter()
            .map(|o| (o.area, o.result.map_err(|e| e.to_string())))
            .collect(),
            Err(error) => {
                let message = describe(&error);
                targets
                    .into_iter()
                    .map(|area| (area, Err(message.clone())))
                    .collect()
            }
        })
    }

    pub(super) fn restore(&self, area: AreaName, id: SnapshotId, path: Option<String>) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let result =
            restore_snapshot(backend, &area, id, path.as_deref()).map_err(|e| e.to_string());
        Msg::Restored {
            area,
            id,
            path,
            result,
        }
    }

    pub(super) fn restore_point(&self, point: snapall_core::RestorePoint) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let name = point.point.to_string();
        let done = restore_point(backend, &point, fresh_point(backend, unix_now()));
        Msg::PointRestored {
            point: name,
            result: Ok(outcome_of(&done)),
        }
    }

    pub(super) fn protect(&self, mount_point: String) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let title = coverage::describe(&mount_point).title;
        let result = protect_area(backend, &mount_point)
            .map_err(|error| explain::failure_text(&title, &error));
        Msg::Protected {
            mount_point,
            result,
        }
    }

    pub(super) fn delete(&self, area: AreaName, ids: Vec<SnapshotId>) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let results = ids
            .into_iter()
            .map(|id| {
                let result = delete_snapshot(backend, &area, id).map_err(|e| describe(&e));
                (id, result)
            })
            .collect();
        Msg::Deleted { area, results }
    }
}
