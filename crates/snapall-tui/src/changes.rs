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
        _ => return None,
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
        _ => return None,
    })
}

/// How a change went, from the message that says so: what came of it, or why it failed.
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
        Msg::JobControlled { result, .. } => result.clone().map(|()| String::new()),
        Msg::GroupsSaved { result, .. } => result.clone().map(|()| String::new()),
        Msg::LimitsSet { result, .. } => result.clone().map(|()| String::new()),
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
        _ => Ok(String::new()),
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
}
