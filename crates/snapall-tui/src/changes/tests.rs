// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a change is called, handed to the background, and how it went.

#![cfg(test)]

use crate::model::{Effect, Msg};

use super::*;
use snapall_core::{AreaName, Description};

fn names(list: &[&str]) -> Vec<AreaName> {
    list.iter().map(|n| AreaName::parse(n).unwrap()).collect()
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
    for reading in [Effect::LoadHealth, Effect::LoadHistory, Effect::LoadAreas] {
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
    let home = AreaName::parse("home").unwrap();
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
                area: home.clone(),
                ids: vec![id],
            },
            Msg::Deleted {
                area: home.clone(),
                results: vec![(id, Ok(()))],
            },
            Msg::Deleted {
                area: home.clone(),
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
                area: home.clone(),
                options: vec![],
                before: limits,
            },
            Msg::LimitsSet {
                area: home.clone(),
                result: Ok(()),
            },
            Msg::LimitsSet {
                area: home.clone(),
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
            Effect::StopProtecting { area: home.clone() },
            Msg::ProtectionStopped {
                area: home.clone(),
                result: Ok(3),
            },
            Msg::ProtectionStopped {
                area: home.clone(),
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
            matches!(
                way_back(&effect, &went_through),
                Some(Rollback::Undo(_) | Rollback::Advice(_))
            ),
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
