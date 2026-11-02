// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a change is called: in the past tense for the Activity tab's record, in the present tense while it runs.

use crate::model::Effect;

/// What an effect that changes something is called in the Activity tab; `None` for one that only looks.
// Exhaustive on purpose: a new kind of change must be given its words and its result here, and the compiler
// refuses until it is. A catch-all would let it through as "not a change" or as a success, unseen.
#[deny(clippy::wildcard_enum_match_arm)]
pub(crate) fn describe_change(effect: &Effect) -> Option<String> {
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    Some(match effect {
        Effect::Create { targets, .. } => format!(
            "Took a snapshot of {}",
            targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Effect::Delete { area, ids } => {
            format!(
                "Deleted {} snapshot{} of {area}",
                ids.len(),
                plural(ids.len())
            )
        }
        Effect::Restore {
            area,
            id,
            path: Some(path),
            ..
        } => {
            format!("Restored {path} from snapshot #{id} of {area}")
        }
        Effect::Restore {
            area,
            id,
            path: None,
            ..
        } => {
            format!("Put {area} back to snapshot #{id}")
        }
        Effect::RestorePoint { point } => {
            format!("Put every area back to restore point {}", point.point)
        }
        Effect::Protect { mount_point } => {
            let title = snapall_core::protecting::coverage::describe(mount_point).title;
            format!("Protected {title} ({mount_point})")
        }
        Effect::SwitchOnCleanup => "Switched on automatic cleanup".to_owned(),
        Effect::SaveGroups { .. } => "Saved the groups of areas".to_owned(),
        Effect::StopProtecting { area } => format!("Stopped protecting {area}"),
        Effect::SetLimits { area, .. } => {
            format!("Changed how many snapshots {area} keeps")
        }
        Effect::ControlJob { unit, action } => {
            use snapall_core::JobAction::{DisableNow, EnableNow, Restart, Start, Stop};
            let did = match action {
                EnableNow => "Switched on",
                DisableNow => "Switched off",
                Start => "Started",
                Stop => "Stopped",
                Restart => "Restarted",
            };
            format!("{did} {}", crate::model::job_title(unit))
        }
        Effect::SetQuotas { turn_on, .. } => {
            format!(
                "Switched room counting {}",
                if *turn_on { "on" } else { "off" }
            )
        }
        // reading changes nothing; and asking a running change to stop is a request, not a change of its own:
        // the change it is about carries the record, and ends as stopped
        Effect::StopChange { .. }
        | Effect::LoadAreas
        | Effect::LoadSnapshots(_)
        | Effect::LoadChanges { .. }
        | Effect::LoadPointOffer { .. }
        | Effect::LoadHealth
        | Effect::LoadGroups
        | Effect::LoadUnprotectPlan { .. }
        | Effect::LoadLimitsData { .. }
        | Effect::LoadHistory
        | Effect::LoadPoints
        | Effect::LoadPointPreview { .. }
        | Effect::LoadFileVersions { .. } => return None,
    })
}

/// What a change is called while it is being done: the same changes as [`describe_change`], in the
/// present tense, for the line that says what the window is doing and for how long.
// Exhaustive on purpose: a new kind of change must be given its words and its result here, and the compiler
// refuses until it is. A catch-all would let it through as "not a change" or as a success, unseen.
#[deny(clippy::wildcard_enum_match_arm)]
pub(crate) fn describe_now(effect: &Effect) -> Option<String> {
    use snapall_core::JobAction::{DisableNow, EnableNow, Restart, Start, Stop};
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    Some(match effect {
        Effect::Create { targets, .. } => format!(
            "Taking a snapshot of {}",
            targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Effect::Delete { area, ids } => {
            format!(
                "Deleting {} snapshot{} of {area}",
                ids.len(),
                plural(ids.len())
            )
        }
        Effect::Restore {
            area,
            id,
            path: Some(path),
            ..
        } => {
            format!("Restoring {path} from snapshot #{id} of {area}")
        }
        Effect::Restore {
            area,
            id,
            path: None,
            ..
        } => {
            format!("Putting {area} back to snapshot #{id}")
        }
        Effect::RestorePoint { point } => {
            format!("Putting every area back to restore point {}", point.point)
        }
        Effect::Protect { mount_point } => {
            let title = snapall_core::protecting::coverage::describe(mount_point).title;
            format!("Protecting {title} ({mount_point})")
        }
        Effect::SwitchOnCleanup => "Switching on automatic cleanup".to_owned(),
        Effect::SaveGroups { .. } => "Saving the groups of areas".to_owned(),
        Effect::StopProtecting { area } => format!("Stopping protection of {area}"),
        Effect::SetLimits { area, .. } => {
            format!("Changing how many snapshots {area} keeps")
        }
        Effect::ControlJob { unit, action } => {
            let doing = match action {
                EnableNow => "Switching on",
                DisableNow => "Switching off",
                Start => "Starting",
                Stop => "Stopping",
                Restart => "Restarting",
            };
            format!("{doing} {}", crate::model::job_title(unit))
        }
        Effect::SetQuotas { turn_on, .. } => {
            format!(
                "Switching room counting {}",
                if *turn_on { "on" } else { "off" }
            )
        }
        // reading changes nothing; and asking a running change to stop is a request, not a change of its own:
        // the change it is about carries the record, and ends as stopped
        Effect::StopChange { .. }
        | Effect::LoadAreas
        | Effect::LoadSnapshots(_)
        | Effect::LoadChanges { .. }
        | Effect::LoadPointOffer { .. }
        | Effect::LoadHealth
        | Effect::LoadGroups
        | Effect::LoadUnprotectPlan { .. }
        | Effect::LoadLimitsData { .. }
        | Effect::LoadHistory
        | Effect::LoadPoints
        | Effect::LoadPointPreview { .. }
        | Effect::LoadFileVersions { .. } => return None,
    })
}
