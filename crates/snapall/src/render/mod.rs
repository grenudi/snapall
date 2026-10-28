// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation: pure functions from domain values to text or JSON.

mod activity;
mod changing;
mod groups;
mod health;
mod listing;
mod points;
mod taking;
#[cfg(test)]
mod tests;

pub use activity::{activity, activity_json};
pub use changing::{changes, changes_json, deleted, deleted_json, restored, restored_json};
pub use groups::{groups, groups_json};
pub use health::{doctor, doctor_json, health, health_json, setup_json};
pub use listing::{areas_json, history, history_json, protected_areas, snapshots, snapshots_json};
pub use points::{
    point_preview, point_preview_json, point_restored, point_restored_json, points, points_json,
};
pub use taking::{dry_run, dry_run_json, report, report_json};

use taking::{kind_word, status_label, verb};
