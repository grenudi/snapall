// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The way back from each kind of change, in one place: what it names as the earlier state, the commands that
//! return to it, and what doing that weighs. The command line and the window both record these, so a change
//! made in either has the same way back, worded the same, and the wording cannot drift between them.

use crate::keeping::groups::GroupName;
use crate::keeping::limits::Limits;
use crate::looking::journal::{Rollback, Undo};
use crate::taking::run::Report;
use crate::vocabulary::consequence::Weight;
use crate::vocabulary::domain::{AreaName, SnapshotId};
use crate::vocabulary::explain::{DELETE_ADVICE, UNPROTECT_ADVICE};

fn undo(what: String, commands: Vec<Vec<String>>, weight: Weight) -> Rollback {
    Rollback::Undo(Undo {
        what,
        commands,
        weight,
    })
}

fn command(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// After taking snapshots: deleting the ones that were taken. That is a deletion like any other, so it weighs
/// what a deletion weighs, and the way back says plainly that they are lost for good.
pub fn after_taking(report: &Report) -> Option<Rollback> {
    let made: Vec<(AreaName, SnapshotId)> = report
        .outcomes
        .iter()
        .filter_map(|outcome| {
            outcome
                .result
                .as_ref()
                .ok()
                .map(|id| (outcome.area.clone(), *id))
        })
        .collect();
    after_taking_ids(&made)
}

/// [`after_taking`], from the snapshots that were taken.
pub fn after_taking_ids(made: &[(AreaName, SnapshotId)]) -> Option<Rollback> {
    if made.is_empty() {
        return None;
    }
    let list: Vec<String> = made
        .iter()
        .map(|(config, id)| format!("#{id} of {config}"))
        .collect();
    Some(undo(
        format!(
            "Delete the {} snapshot{} this took ({}). They are lost for good, but nothing else changes",
            made.len(),
            if made.len() == 1 { "" } else { "s" },
            list.join(", ")
        ),
        made.iter()
            .map(|(config, id)| command(&["delete", config.as_str(), &id.to_string()]))
            .collect(),
        Weight::Destroys,
    ))
}

/// After putting one area (or one path of it) back: going back to the snapshot of the state just before.
pub fn after_restoring(config: &AreaName, path: Option<&str>, safety: SnapshotId) -> Rollback {
    let mut back = command(&["restore", config.as_str(), &safety.to_string()]);
    back.extend(path.map(str::to_owned));
    undo(
        format!(
            "Put {config}{} back to how it was just before: snapshot #{safety} (taken by this restore)",
            path.map_or(String::new(), |path| format!(" ({path})"))
        ),
        vec![back],
        Weight::Changes,
    )
}

/// After putting every area of a restore point back: going back to the restore point of the states just before.
pub fn after_restoring_all(safety_point: &str) -> Rollback {
    undo(
        format!("Put every area back to how it was just before: restore point {safety_point} (taken by this restore)"),
        vec![command(&["restore-all", safety_point])],
        Weight::Changes,
    )
}

/// After protecting parts of the disk: not protecting them again, which deletes the snapshots taken since.
pub fn after_protecting(names: &[String]) -> Rollback {
    undo(
        format!(
            "Stop protecting {} again, back to not protected. Any snapshots taken since would be deleted",
            names.join(", ")
        ),
        names.iter().map(|name| command(&["unprotect", name])).collect(),
        Weight::Destroys,
    )
}

/// After changing what an area keeps: exactly what it kept before.
pub fn after_limits(config: &str, before: &Limits) -> Rollback {
    undo(
        format!("Set {config} back to: {}", before.describe()),
        vec![vec![
            "limits".to_owned(),
            config.to_owned(),
            "--keep".to_owned(),
            before.keep.to_string(),
            "--min-age".to_owned(),
            (before.min_age_seconds / 60).to_string(),
            "--cleanup".to_owned(),
            if before.cleanup { "on" } else { "off" }.to_owned(),
        ]],
        Weight::Changes,
    )
}

/// After switching room counting: switching it back.
pub fn after_room_counting(turned_on: bool) -> Rollback {
    let before = if turned_on { "off" } else { "on" };
    undo(
        format!("Switch counting the room snapshots take back {before}, as it was"),
        vec![command(&["quotas", before])],
        Weight::Changes,
    )
}

/// After changing a group: the group as it was (made again with its old areas), or, if it was new, taken away.
pub fn after_groups(name: &GroupName, before: Option<&[AreaName]>) -> Rollback {
    match before {
        Some(areas) => {
            let list = areas
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            undo(
                format!("Put the group {name} back to: {}", list.replace(',', ", ")),
                vec![command(&["groups", "add", name.as_str(), &list])],
                Weight::Changes,
            )
        }
        None => undo(
            format!("Take the group {name} away again: it did not exist before"),
            vec![command(&["groups", "remove", name.as_str()])],
            Weight::Changes,
        ),
    }
}

/// After deleting a snapshot: nothing brings it back, and the record says what protects next time.
pub fn after_deleting() -> Rollback {
    Rollback::Advice(DELETE_ADVICE.to_owned())
}

/// After an area stopped being protected: the same.
pub fn after_unprotecting() -> Rollback {
    Rollback::Advice(UNPROTECT_ADVICE.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::testing::InMemoryBackend;
    use crate::taking::run::{plan_snapshots, run_plan};
    use crate::vocabulary::domain::Description;

    fn area(name: &str) -> AreaName {
        AreaName::parse(name).unwrap()
    }

    fn the_undo(rollback: Rollback) -> Undo {
        match rollback {
            Rollback::Undo(undo) => undo,
            Rollback::Advice(text) => panic!("expected a way back, got advice: {text}"),
        }
    }

    #[test]
    fn taking_snapshots_is_undone_by_deleting_exactly_those_and_weighs_a_deletion() {
        let backend = InMemoryBackend::with_areas(&["data", "home"]);
        let report = run_plan(
            &backend,
            &plan_snapshots(&backend, None).unwrap(),
            &Description::parse("x").unwrap(),
            None,
        );
        let undo = the_undo(after_taking(&report).unwrap());
        assert_eq!(undo.weight, Weight::Destroys);
        assert_eq!(undo.commands.len(), 2);
        assert!(undo
            .commands
            .iter()
            .all(|c| c[0] == "delete" && c[2].parse::<u64>().is_ok()));
        assert!(
            undo.what.starts_with("Delete the 2 snapshots this took (#"),
            "{}",
            undo.what
        );
        assert!(undo.what.contains("lost for good"));
        let nothing = crate::taking::run::Report { outcomes: vec![] };
        assert!(
            after_taking(&nothing).is_none(),
            "nothing taken, nothing to undo"
        );
    }

    #[test]
    fn a_restore_goes_back_to_the_snapshot_of_just_before_with_its_path_if_it_had_one() {
        let safety = SnapshotId::new(12).unwrap();
        let whole = the_undo(after_restoring(&area("home"), None, safety));
        assert_eq!(whole.commands, [["restore", "home", "12"]]);
        assert_eq!(whole.weight, Weight::Changes);
        assert!(whole
            .what
            .contains("just before: snapshot #12 (taken by this restore)"));
        let one = the_undo(after_restoring(
            &area("home"),
            Some("/home/a b/n.txt"),
            safety,
        ));
        assert_eq!(
            one.commands,
            [["restore", "home", "12", "/home/a b/n.txt"]],
            "the path is one word, spaces and all"
        );
        let all = the_undo(after_restoring_all("20261025T181500Z"));
        assert_eq!(all.commands, [["restore-all", "20261025T181500Z"]]);
    }

    #[test]
    fn settings_go_back_to_exactly_what_they_were() {
        let before = Limits {
            keep: 50,
            min_age_seconds: 1800,
            cleanup: true,
        };
        let undo = the_undo(after_limits("home", &before));
        assert_eq!(
            undo.commands,
            [[
                "limits",
                "home",
                "--keep",
                "50",
                "--min-age",
                "30",
                "--cleanup",
                "on"
            ]]
        );
        assert!(undo.what.contains(
            "keeps the newest 50, never removes one younger than 30 minutes, cleanup on"
        ));
        assert_eq!(
            the_undo(after_room_counting(true)).commands,
            [["quotas", "off"]]
        );
        assert_eq!(
            the_undo(after_room_counting(false)).commands,
            [["quotas", "on"]]
        );
    }

    #[test]
    fn a_group_goes_back_to_its_old_areas_or_is_taken_away_if_it_was_new() {
        let name = GroupName::parse("theming").unwrap();
        let old = [area("data"), area("home")];
        let changed = the_undo(after_groups(&name, Some(&old)));
        assert_eq!(
            changed.commands,
            [["groups", "add", "theming", "data,home"]]
        );
        assert!(changed.what.contains("back to: data, home"));
        let made = the_undo(after_groups(&name, None));
        assert_eq!(made.commands, [["groups", "remove", "theming"]]);
    }

    #[test]
    fn protecting_is_undone_by_not_protecting_which_weighs_a_deletion() {
        let undo = the_undo(after_protecting(&["games".to_owned(), "notes".to_owned()]));
        assert_eq!(
            undo.commands,
            [["unprotect", "games"], ["unprotect", "notes"]]
        );
        assert_eq!(undo.weight, Weight::Destroys);
    }

    #[test]
    fn what_cannot_come_back_says_so_and_what_protects_next_time() {
        for advice in [after_deleting(), after_unprotecting()] {
            match advice {
                Rollback::Advice(text) => {
                    assert!(text.contains("snapall create --group NAME"), "{text}")
                }
                Rollback::Undo(_) => panic!("cannot be undone"),
            }
        }
    }
}
