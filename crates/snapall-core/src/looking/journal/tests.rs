// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall did, remembered.

#![cfg(test)]

use super::*;

fn started(id: &str, what: &str, pid: u32) -> Event {
    Event::Started {
        id: id.into(),
        at: "2026-10-25 18:15:00".into(),
        what: what.into(),
        source: Source::Window,
        pid,
    }
}

fn finished(id: &str, outcome: Result<&str, &str>) -> Event {
    Event::Finished {
        id: id.into(),
        at: "2026-10-25 18:15:03".into(),
        outcome: outcome.map(str::to_owned).map_err(str::to_owned).into(),
        took_ms: 3000,
    }
}

#[test]
fn a_change_that_started_and_ended_is_done_or_failed_with_what_came_of_it() {
    let entries = fold(
        &[
            started("a", "Took a snapshot of home", 10),
            finished("a", Ok("1 snapshot taken")),
            started("b", "Deleted 1 snapshot of home", 10),
            finished("b", Err("snapshot #3 does not exist")),
        ],
        |_| true,
    );
    assert_eq!(entries.len(), 2);
    assert_eq!(
        (entries[0].id.as_str(), entries[0].status),
        ("b", Status::Failed),
        "newest first"
    );
    assert_eq!(entries[0].detail, "snapshot #3 does not exist");
    assert_eq!(
        (entries[1].status, entries[1].detail.as_str()),
        (Status::Done, "1 snapshot taken")
    );
    assert_eq!(entries[1].took_ms, Some(3000));
    assert_eq!(
        entries[1].finished_at.as_deref(),
        Some("2026-10-25 18:15:03")
    );
}

#[test]
fn a_change_that_started_and_never_ended_is_running_if_its_process_is_and_interrupted_if_not() {
    let events = [started("a", "Putting Home back", 4242)];
    assert_eq!(fold(&events, |pid| pid == 4242)[0].status, Status::Running);
    let gone = fold(&events, |_| false);
    assert_eq!(gone[0].status, Status::Interrupted);
    assert!(
        gone[0]
            .detail
            .contains("stopped before it could say how it ended"),
        "{}",
        gone[0].detail
    );
    assert!(
        gone[0].detail.contains("Look at the snapshots"),
        "it says what to do next"
    );
}

#[test]
fn a_change_that_did_end_is_never_called_interrupted_whatever_its_process() {
    let events = [started("a", "x", 1), finished("a", Ok(""))];
    assert_eq!(fold(&events, |_| false)[0].status, Status::Done);
}

#[test]
fn an_end_whose_start_was_rotated_away_is_still_shown_and_a_repeated_start_changes_nothing() {
    let alone = fold(&[finished("old", Ok("done"))], |_| false);
    assert_eq!(
        (alone[0].what.as_str(), alone[0].status),
        ("An earlier change", Status::Done)
    );
    let twice = fold(
        &[
            started("a", "x", 1),
            started("a", "x", 1),
            finished("a", Ok("")),
        ],
        |_| true,
    );
    assert_eq!(twice.len(), 1);
}

#[test]
fn interleaved_changes_of_different_programs_are_kept_apart_by_their_ids() {
    let entries = fold(
        &[
            started("w", "Window change", 1),
            started("c", "Command change", 2),
            finished("c", Ok("done")),
            finished("w", Err("boom")),
        ],
        |_| true,
    );
    let by = |id: &str| entries.iter().find(|e| e.id == id).unwrap().status;
    assert_eq!((by("w"), by("c")), (Status::Failed, Status::Done));
}

#[test]
fn the_words_for_a_source_round_trip() {
    for source in [Source::Window, Source::Command] {
        assert_eq!(Source::from_word(source.word()), Some(source));
    }
    assert_eq!(Source::from_word("other"), None);
}

#[test]
fn a_way_back_recorded_for_a_change_is_part_of_its_entry_whenever_it_was_recorded() {
    let undo = Rollback::Undo(Undo {
        what: "Put Home back to how it was: snapshot #12".into(),
        commands: vec![vec!["restore".into(), "home".into(), "12".into()]],
        weight: crate::vocabulary::consequence::Weight::Changes,
    });
    let way_back = |rollback: Rollback| Event::Rollback {
        id: "a".into(),
        at: "t".into(),
        rollback,
    };
    // before the change ended, and after it
    for events in [
        vec![
            started("a", "x", 1),
            way_back(undo.clone()),
            finished("a", Ok("")),
        ],
        vec![
            started("a", "x", 1),
            finished("a", Ok("")),
            way_back(undo.clone()),
        ],
    ] {
        assert_eq!(fold(&events, |_| true)[0].rollback, Some(undo.clone()));
    }
    let advice = Rollback::Advice("Deleted snapshots are gone.".into());
    let entries = fold(&[started("b", "y", 1), way_back(advice.clone())], |_| true);
    assert_eq!(
        entries[0].rollback, None,
        "advice for another change is not this one's"
    );
    assert!(
        fold(&[way_back(advice)], |_| true).is_empty(),
        "a way back alone is not a change"
    );
}

#[test]
fn a_way_back_is_worded_once_for_whoever_shows_it() {
    let undo = Rollback::Undo(Undo {
        what: "Put Home back to how it was".into(),
        commands: vec![],
        weight: crate::vocabulary::consequence::Weight::Changes,
    });
    assert_eq!(
        undo.words("42-1-c"),
        [
            "Way back: Put Home back to how it was.",
            "To do it: snapall undo 42-1-c"
        ]
    );
    let advice = Rollback::Advice("Deleted snapshots are gone.".into());
    assert_eq!(
        advice.words("42-1-c"),
        ["It cannot be undone. Deleted snapshots are gone."]
    );
}

#[test]
fn an_outcome_is_stored_as_a_word_and_read_back_and_an_unknown_word_is_refused() {
    for outcome in [
        Outcome::Done("took 2 snapshots".to_owned()),
        Outcome::Failed("no room".to_owned()),
        Outcome::Cancelled("stopped after 1 of 3".to_owned()),
    ] {
        let again = Outcome::from_word(outcome.word(), outcome.detail().to_owned());
        assert_eq!(again, Some(outcome));
    }
    assert_eq!(Outcome::from_word("exploded", String::new()), None);
}

#[test]
fn a_source_is_stored_in_one_word_and_read_by_a_person_in_another() {
    // the stored words are in journals already written, so they must not change; the label is for people
    assert_eq!(Source::Command.word(), "command");
    assert_eq!(Source::Command.label(), "command line");
    assert_eq!(Source::Window.word(), "window");
    assert_eq!(
        Source::from_word(Source::Command.word()),
        Some(Source::Command)
    );
}
