// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a change went, and the way back from it, read from what came back.

use crate::model::{Effect, Msg, PointOutcome};
use snapall_core::Counting;

/// What a person is told about putting a restore point back, from what happened to each area.
pub(crate) fn outcome_of(done: &snapall_core::PointRestore) -> PointOutcome {
    use snapall_core::MemberResult;
    let count = |wanted: fn(&MemberResult) -> bool| {
        done.members.iter().filter(|m| wanted(&m.result)).count()
    };
    PointOutcome {
        restored: count(|r| matches!(r, MemberResult::Restored(_))),
        unchanged: count(|r| matches!(r, MemberResult::AlreadyThere)),
        failed: done
            .members
            .iter()
            .filter_map(|m| match &m.result {
                MemberResult::Failed(why) => Some(format!("{}: {why}", m.area)),
                _ => None,
            })
            .collect(),
        undo: done.changed_anything().then(|| done.safety.to_string()),
    }
}

/// How a change went, from the message that says so: what came of it, or why it failed.
// Exhaustive on purpose: a new kind of change must be given its words and its result here, and the compiler
// refuses until it is. A catch-all would let it through as "not a change" or as a success, unseen.
#[deny(clippy::wildcard_enum_match_arm)]
pub(crate) fn how_it_went(msg: &Msg) -> Result<String, String> {
    let failures = |pairs: Vec<(String, &Result<String, String>)>| -> Vec<String> {
        pairs
            .into_iter()
            .filter_map(|(what, result)| result.as_ref().err().map(|why| format!("{what}: {why}")))
            .collect()
    };
    match msg {
        Msg::Created(results) => {
            let each: Vec<(String, Result<String, String>)> = results
                .iter()
                .map(|(area, result)| {
                    (
                        area.to_string(),
                        result
                            .as_ref()
                            .map(|id| format!("#{id}"))
                            .map_err(Clone::clone),
                    )
                })
                .collect();
            let failed = failures(
                each.iter()
                    .map(|(name, result)| (name.clone(), result))
                    .collect(),
            );
            if failed.is_empty() {
                Ok(format!(
                    "{} snapshot{} taken",
                    results.len(),
                    if results.len() == 1 { "" } else { "s" }
                ))
            } else {
                Err(failed.join("; "))
            }
        }
        Msg::Deleted { results, .. } => {
            let failed: Vec<String> = results
                .iter()
                .filter_map(|(id, result)| result.as_ref().err().map(|why| format!("#{id}: {why}")))
                .collect();
            if failed.is_empty() {
                Ok(String::new())
            } else {
                Err(failed.join("; "))
            }
        }
        Msg::Restored { result, .. } => result
            .as_ref()
            .map(|done| format!("How it was before is snapshot #{}", done.safety_snapshot))
            .map_err(Clone::clone),
        Msg::PointRestored {
            result: Ok(outcome),
            ..
        } if outcome.failed.is_empty() => Ok(format!(
            "{} area{} put back.{}",
            outcome.restored,
            if outcome.restored == 1 { "" } else { "s" },
            outcome.undo.as_ref().map_or(String::new(), |undo| format!(
                " To undo it: restore point {undo}"
            ))
        )),
        Msg::PointRestored {
            result: Ok(outcome),
            ..
        } => Err(outcome.failed.join("; ")),
        Msg::PointRestored {
            result: Err(why), ..
        }
        | Msg::CleanupSwitched(Err(why)) => Err(why.clone()),
        Msg::CleanupSwitched(Ok(())) => Ok(String::new()),
        Msg::JobControlled { result, .. } => result.clone().map(|()| String::new()),
        Msg::GroupsSaved { result, .. } => result.clone().map(|()| String::new()),
        Msg::LimitsSet { result, .. } => result.clone().map(|()| String::new()),
        Msg::ProtectionStopped { result, .. } => {
            result.clone().map(|n| format!("{n} snapshots deleted"))
        }
        Msg::QuotasSet {
            to: Counting::On,
            result,
        } => result
            .clone()
            .map(|()| "The disk is building its tally in the background".to_owned()),
        Msg::QuotasSet { result, .. } => result.clone().map(|()| String::new()),
        Msg::Protected { result, .. } => result
            .as_ref()
            .map(|_| "It has snapshots from now on.".to_owned())
            .map_err(Clone::clone),
        // everything else carries no result of a change: it is read, shown or looked at. Listed one by
        // one, so that a new message that DOES carry a result cannot be taken for a success by default.
        Msg::Start
        | Msg::Tick
        | Msg::Key(..)
        | Msg::Hover(..)
        | Msg::Click(..)
        | Msg::Scroll { .. }
        | Msg::ChangesLoaded { .. }
        | Msg::FileVersionsLoaded { .. }
        | Msg::AreasLoaded(..)
        | Msg::PointOfferLoaded { .. }
        | Msg::Refused(..)
        | Msg::LimitsData { .. }
        | Msg::UnprotectPlanLoaded { .. }
        | Msg::GroupsLoaded(..)
        | Msg::StartedInBackground { .. }
        | Msg::StopAsked { .. }
        | Msg::HistoryLoaded(..)
        | Msg::HealthLoaded(..)
        | Msg::Finished(..)
        | Msg::PointsLoaded(..)
        | Msg::PointPreviewLoaded(..)
        | Msg::SnapshotsLoaded { .. } => Ok(String::new()),
    }
}

/// How to get back from what the window just did, worked out from what it asked for and what came of it:
/// only for a change that was made, and in the same words the command line uses.
pub(crate) fn way_back(effect: &Effect, msg: &Msg) -> Option<snapall_core::Rollback> {
    use snapall_core::going_back::rollback as back;
    use snapall_core::Rollback;
    match (effect, msg) {
        (Effect::Create { .. }, Msg::Created(made)) => {
            let ids: Vec<_> = made
                .iter()
                .filter_map(|(area, result)| result.as_ref().ok().map(|id| (area.clone(), *id)))
                .collect();
            back::after_taking_ids(&ids)
        }
        (Effect::Delete { .. }, Msg::Deleted { results, .. })
            if results.iter().any(|(_, result)| result.is_ok()) =>
        {
            Some(back::after_deleting())
        }
        (Effect::Restore { area, path, .. }, Msg::Restored { result: Ok(done), .. }) => {
            Some(back::after_restoring(area, path.as_deref(), done.safety_snapshot))
        }
        (Effect::Protect { .. }, Msg::Protected { result: Ok(name), .. }) => {
            Some(back::after_protecting(&[name.to_string()]))
        }
        (Effect::SetQuotas { to, .. }, Msg::QuotasSet { result: Ok(()), .. }) => {
            Some(back::after_room_counting(*to))
        }
        (Effect::SetLimits { area, before, .. }, Msg::LimitsSet { result: Ok(()), .. }) => {
            Some(back::after_limits(area.as_str(), before))
        }
        (Effect::SaveGroups { groups, before }, Msg::GroupsSaved { result: Ok(()), .. }) => {
            // a save changes one group: the first whose areas differ is the one to put back
            let mut names: Vec<&snapall_core::GroupName> = groups.names().chain(before.names()).collect();
            names.dedup();
            names
                .into_iter()
                .find(|name| groups.get(name) != before.get(name))
                .map(|name| back::after_groups(name, before.get(name)))
        }
        (Effect::StopProtecting { .. }, Msg::ProtectionStopped { result: Ok(_), .. }) => {
            Some(back::after_unprotecting())
        }
        (Effect::SwitchOnCleanup, Msg::CleanupSwitched(Ok(()))) => Some(Rollback::Advice(
            "Automatic cleanup was off. To switch it off again, open its row under Background jobs in the Health tab and choose the opposite."
                .to_owned(),
        )),
        (Effect::ControlJob { .. }, Msg::JobControlled { result: Ok(()), .. }) => Some(Rollback::Advice(
            "To put the job back as it was, open it again in the Health tab and choose the opposite action."
                .to_owned(),
        )),
        _ => None,
    }
}
