// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a change is called in the Activity tab, and how it went: the words, apart from the doing.

use crate::model::{Effect, Msg, PointOutcome};

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
                MemberResult::Failed(why) => Some(format!("{}: {why}", m.config)),
                _ => None,
            })
            .collect(),
        undo: done.changed_anything().then(|| done.safety.to_string()),
    }
}

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
        Effect::Delete { config, ids } => {
            format!(
                "Deleted {} snapshot{} of {config}",
                ids.len(),
                plural(ids.len())
            )
        }
        Effect::Restore {
            config,
            id,
            path: Some(path),
            ..
        } => {
            format!("Restored {path} from snapshot #{id} of {config}")
        }
        Effect::Restore {
            config,
            id,
            path: None,
            ..
        } => {
            format!("Put {config} back to snapshot #{id}")
        }
        Effect::RestorePoint { point } => {
            format!("Put every area back to restore point {}", point.point)
        }
        Effect::Protect { mount_point } => {
            let title = snapall_core::coverage::describe(mount_point).title;
            format!("Protected {title} ({mount_point})")
        }
        Effect::SwitchOnCleanup => "Switched on automatic cleanup".to_owned(),
        Effect::SaveGroups { .. } => "Saved the groups of areas".to_owned(),
        Effect::StopProtecting { config } => format!("Stopped protecting {config}"),
        Effect::SetLimits { config, .. } => {
            format!("Changed how many snapshots {config} keeps")
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
        | Effect::LoadConfigs
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

/// The command line that does a change on its own, for the changes slow enough, and made of enough
/// steps, to be worth handing to a process that goes on without the window: a snapshot of several areas,
/// and a restore point put back area by area. A change of one area takes a moment and stays here.
pub(crate) fn background_arguments(effect: &Effect) -> Option<Vec<String>> {
    match effect {
        Effect::Create {
            targets,
            description,
        } if targets.len() > 1 => Some(vec![
            "create".to_owned(),
            "--only".to_owned(),
            targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            // free text: after the `--`, so that a note which begins with a dash stays a note
            "--".to_owned(),
            description.to_string(),
        ]),
        Effect::RestorePoint { point } => Some(vec![
            "restore-all".to_owned(),
            point.point.to_string(),
            "--yes".to_owned(),
        ]),
        _ => None,
    }
}

/// A name for one change that no other is likely to have: the program making it, when, and which of its
/// changes it is.
pub(crate) fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let milliseconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    format!(
        "{}-{milliseconds}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed)
    )
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
        Effect::Delete { config, ids } => {
            format!(
                "Deleting {} snapshot{} of {config}",
                ids.len(),
                plural(ids.len())
            )
        }
        Effect::Restore {
            config,
            id,
            path: Some(path),
            ..
        } => {
            format!("Restoring {path} from snapshot #{id} of {config}")
        }
        Effect::Restore {
            config,
            id,
            path: None,
            ..
        } => {
            format!("Putting {config} back to snapshot #{id}")
        }
        Effect::RestorePoint { point } => {
            format!("Putting every area back to restore point {}", point.point)
        }
        Effect::Protect { mount_point } => {
            let title = snapall_core::coverage::describe(mount_point).title;
            format!("Protecting {title} ({mount_point})")
        }
        Effect::SwitchOnCleanup => "Switching on automatic cleanup".to_owned(),
        Effect::SaveGroups { .. } => "Saving the groups of areas".to_owned(),
        Effect::StopProtecting { config } => format!("Stopping protection of {config}"),
        Effect::SetLimits { config, .. } => {
            format!("Changing how many snapshots {config} keeps")
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
        | Effect::LoadConfigs
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
                .map(|(config, result)| {
                    (
                        config.to_string(),
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
            turn_on: true,
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
        | Msg::ConfigsLoaded(..)
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
    use snapall_core::rollback as back;
    use snapall_core::Rollback;
    match (effect, msg) {
        (Effect::Create { .. }, Msg::Created(made)) => {
            let ids: Vec<_> = made
                .iter()
                .filter_map(|(config, result)| result.as_ref().ok().map(|id| (config.clone(), *id)))
                .collect();
            back::after_taking_ids(&ids)
        }
        (Effect::Delete { .. }, Msg::Deleted { results, .. })
            if results.iter().any(|(_, result)| result.is_ok()) =>
        {
            Some(back::after_deleting())
        }
        (Effect::Restore { config, path, .. }, Msg::Restored { result: Ok(done), .. }) => {
            Some(back::after_restoring(config, path.as_deref(), done.safety_snapshot))
        }
        (Effect::Protect { .. }, Msg::Protected { result: Ok(name), .. }) => {
            Some(back::after_protecting(&[name.to_string()]))
        }
        (Effect::SetQuotas { turn_on, .. }, Msg::QuotasSet { result: Ok(()), .. }) => {
            Some(back::after_room_counting(*turn_on))
        }
        (Effect::SetLimits { config, before, .. }, Msg::LimitsSet { result: Ok(()), .. }) => {
            Some(back::after_limits(config.as_str(), before))
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

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::{ConfigName, Description};

    fn names(list: &[&str]) -> Vec<ConfigName> {
        list.iter().map(|n| ConfigName::parse(n).unwrap()).collect()
    }

    #[test]
    fn only_a_change_of_several_areas_is_handed_over_and_as_the_command_that_does_it() {
        let several = Effect::Create {
            targets: names(&["home", "root"]),
            description: Description::parse("--before theming").unwrap(),
        };
        assert_eq!(
            background_arguments(&several),
            Some(
                ["create", "--only", "home,root", "--", "--before theming"]
                    .map(String::from)
                    .to_vec()
            ),
            "the note comes last, after `--`, so that a dash cannot make it an option"
        );
        let one = Effect::Create {
            targets: names(&["home"]),
            description: Description::default(),
        };
        assert_eq!(
            background_arguments(&one),
            None,
            "one area takes a moment: it stays in the window"
        );
        assert_eq!(background_arguments(&Effect::LoadHealth), None);
    }

    #[test]
    fn every_change_has_a_name_in_both_tenses_and_a_reading_has_neither() {
        let create = Effect::Create {
            targets: names(&["home", "root"]),
            description: Description::default(),
        };
        assert_eq!(
            describe_now(&create).as_deref(),
            Some("Taking a snapshot of home, root")
        );
        assert_eq!(
            describe_change(&create).as_deref(),
            Some("Took a snapshot of home, root")
        );
        for reading in [Effect::LoadHealth, Effect::LoadHistory, Effect::LoadConfigs] {
            assert_eq!(
                (describe_now(&reading), describe_change(&reading)),
                (None, None),
                "{reading:?}"
            );
        }
    }

    /// Every change the window can make, with the message that says it went through, and the same with the
    /// message that says it did not. A way back is for a change that was made: never for one that failed.
    #[test]
    fn every_change_the_window_makes_records_its_way_back_and_one_that_failed_records_none() {
        use snapall_core::{GroupName, Groups, JobAction, Limits, Rollback, SnapshotId};
        let home = ConfigName::parse("home").unwrap();
        let id = SnapshotId::new(7).unwrap();
        fn failed<T>() -> Result<T, String> {
            Err("it did not work".to_owned())
        }
        let mut before = Groups::default();
        before
            .set(GroupName::parse("g").unwrap(), vec![home.clone()])
            .unwrap();
        let after = Groups::default();
        let limits = Limits {
            keep: 50,
            min_age_seconds: 1800,
            cleanup: true,
        };

        // (what was asked, what came of it when it went through, what came of it when it did not)
        let changes: Vec<(&str, Effect, Msg, Msg)> = vec![
            (
                "taking snapshots",
                Effect::Create {
                    targets: vec![home.clone()],
                    description: Description::parse("x").unwrap(),
                },
                Msg::Created(vec![(home.clone(), Ok(id))]),
                Msg::Created(vec![(home.clone(), failed())]),
            ),
            (
                "deleting",
                Effect::Delete {
                    config: home.clone(),
                    ids: vec![id],
                },
                Msg::Deleted {
                    config: home.clone(),
                    results: vec![(id, Ok(()))],
                },
                Msg::Deleted {
                    config: home.clone(),
                    results: vec![(id, failed())],
                },
            ),
            (
                "protecting",
                Effect::Protect {
                    mount_point: "/srv".into(),
                },
                Msg::Protected {
                    mount_point: "/srv".into(),
                    result: Ok(home.clone()),
                },
                Msg::Protected {
                    mount_point: "/srv".into(),
                    result: failed(),
                },
            ),
            (
                "room counting",
                Effect::SetQuotas {
                    turn_on: true,
                    paths: vec![],
                },
                Msg::QuotasSet {
                    turn_on: true,
                    result: Ok(()),
                },
                Msg::QuotasSet {
                    turn_on: true,
                    result: failed(),
                },
            ),
            (
                "limits",
                Effect::SetLimits {
                    config: home.clone(),
                    options: vec![],
                    before: limits,
                },
                Msg::LimitsSet {
                    config: home.clone(),
                    result: Ok(()),
                },
                Msg::LimitsSet {
                    config: home.clone(),
                    result: failed(),
                },
            ),
            (
                "groups",
                Effect::SaveGroups {
                    groups: after.clone(),
                    before: before.clone(),
                },
                Msg::GroupsSaved {
                    groups: after.clone(),
                    result: Ok(()),
                },
                Msg::GroupsSaved {
                    groups: after,
                    result: failed(),
                },
            ),
            (
                "stopping protection",
                Effect::StopProtecting {
                    config: home.clone(),
                },
                Msg::ProtectionStopped {
                    config: home.clone(),
                    result: Ok(3),
                },
                Msg::ProtectionStopped {
                    config: home.clone(),
                    result: failed(),
                },
            ),
            (
                "switching cleanup on",
                Effect::SwitchOnCleanup,
                Msg::CleanupSwitched(Ok(())),
                Msg::CleanupSwitched(failed()),
            ),
            (
                "controlling a job",
                Effect::ControlJob {
                    unit: "u.timer".into(),
                    action: JobAction::Start,
                },
                Msg::JobControlled {
                    unit: "u.timer".into(),
                    action: JobAction::Start,
                    result: Ok(()),
                },
                Msg::JobControlled {
                    unit: "u.timer".into(),
                    action: JobAction::Start,
                    result: failed(),
                },
            ),
        ];
        for (what, effect, went_through, did_not) in changes {
            assert!(
                describe_change(&effect).is_some(),
                "{what}: it is a change, so it has words"
            );
            assert!(
                matches!(way_back(&effect, &went_through), Some(Rollback::Undo(_) | Rollback::Advice(_))),
                "{what}: a change that was made must say how to get back from it, or that it cannot"
            );
            assert!(
                way_back(&effect, &did_not).is_none(),
                "{what}: nothing changed, so there is nothing to get back from"
            );
            assert!(how_it_went(&went_through).is_ok(), "{what}");
            assert!(
                how_it_went(&did_not).is_err(),
                "{what}: a failure is recorded as one"
            );
        }
    }
}
