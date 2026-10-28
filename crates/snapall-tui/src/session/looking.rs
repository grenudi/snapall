// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window asks to be looked at, answered from the backend and the machine.

use super::executor::{describe, Executor};
use crate::model::{Msg, PointOffer};
use crate::tabs::health::HealthData;
use snapall_core::protecting::unprotect;
use snapall_core::AreaName;
use snapall_core::SnapshotId;
use snapall_core::{
    all_snapshots, assess, file_versions_of, gather, point_of, preview_point, preview_restore,
    restore_points, snapshots_of, SnapshotBackend,
};

impl Executor<'_> {
    pub(super) fn load_groups(&self) -> Msg {
        Msg::GroupsLoaded(self.system.groups())
    }

    pub(super) fn load_unprotect_plan(&self, area: AreaName) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let plan = self
            .system
            .groups()
            .map_err(|why| why.to_string())
            .and_then(|groups| unprotect::plan(backend, &groups, &area).map_err(|e| e.to_string()));
        Msg::UnprotectPlanLoaded { area, plan }
    }

    pub(super) fn load_limits_data(&self, area: AreaName) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        match backend.list_snapshots(&area) {
            Ok(snapshots) => Msg::LimitsData { area, snapshots },
            Err(error) => Msg::LimitsSet {
                area,
                result: Err(error.to_string()),
            },
        }
    }

    pub(super) fn load_history(&self) -> Msg {
        Msg::HistoryLoaded(self.system.history(200))
    }

    pub(super) fn load_health(&self) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        Msg::HealthLoaded(
            gather(backend, self.system)
                .map(|facts| {
                    let report = assess(&facts);
                    Box::new(HealthData { facts, report })
                })
                .map_err(|error| error.to_string()),
        )
    }

    pub(super) fn load_areas(&self) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        Msg::AreasLoaded(backend.list_areas().map_err(|e| e.to_string()))
    }

    pub(super) fn load_snapshots(&self, area: AreaName) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let result = snapshots_of(backend, &area).map_err(|e| e.to_string());
        Msg::SnapshotsLoaded { area, result }
    }

    pub(super) fn load_changes(&self, area: AreaName, id: SnapshotId, path: Option<String>) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let result =
            preview_restore(backend, &area, id, path.as_deref()).map_err(|e| e.to_string());
        Msg::ChangesLoaded { area, id, result }
    }

    pub(super) fn load_points(&self) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        Msg::PointsLoaded(
            all_snapshots(backend)
                .map(|all| restore_points(&all))
                .map_err(|error| describe(&error)),
        )
    }

    pub(super) fn load_point_preview(&self, point: snapall_core::RestorePoint) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        Msg::PointPreviewLoaded(PointOffer {
            views: preview_point(backend, &point),
            point,
        })
    }

    pub(super) fn load_point_offer(&self, area: AreaName, id: SnapshotId) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        // Is this snapshot one of several taken by one action? Failing to look is not an
        // error to show: the prompt simply does not offer the extra action.
        let offer = all_snapshots(backend).ok().and_then(|all| {
            let points = restore_points(&all);
            point_of(&points, &area, id)
                .cloned()
                .map(|point| PointOffer {
                    views: preview_point(backend, &point),
                    point,
                })
        });
        Msg::PointOfferLoaded { area, id, offer }
    }

    pub(super) fn load_file_versions(&self, area: AreaName, path: String) -> Msg {
        let backend: &dyn SnapshotBackend = self.backend;
        let result = file_versions_of(backend, &area, &path).map_err(|e| e.to_string());
        Msg::FileVersionsLoaded { area, path, result }
    }
}
