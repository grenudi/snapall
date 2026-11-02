// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall tells a person.

#![cfg(test)]

use crate::ports::machine::QuotaMode;
use crate::protecting::coverage::{describe, Area};
use crate::protecting::protect::SetupError;

use super::*;

fn schedule() -> Vec<String> {
    vec![
        "10 minutes after the computer starts".into(),
        "then again 1 day after each run starts".into(),
    ]
}

fn limits() -> Vec<Limit> {
    vec![
        Limit {
            area: "Home".into(),
            keep: Some(50),
            min_age_seconds: Some(1800),
        },
        Limit {
            area: "System".into(),
            keep: Some(20),
            min_age_seconds: None,
        },
    ]
}

const JARGON: [&str; 7] = [
    "subvolume",
    "btrfs",
    "snapper",
    "config",
    "filesystem",
    "file system",
    "mountpoint",
];

fn assert_plain(label: &str, text: &str) {
    let lower = text.to_lowercase();
    for word in JARGON {
        assert!(!lower.contains(word), "`{word}` in {label}: {text}");
    }
}

#[test]
fn nothing_a_person_reads_assumes_they_know_the_vocabulary() {
    assert_plain("snapshot", WHAT_IS_A_SNAPSHOT);
    assert_plain("area", WHAT_IS_AN_AREA);
    assert_plain("backup", NOT_A_BACKUP);
    assert_plain("cleanup offer", &cleanup_offer(&schedule(), &limits()));
    assert_plain("restoring", RESTORING);
    for state in [Cleanup::On, Cleanup::Off, Cleanup::Unavailable] {
        assert_plain("cleanup", &cleanup_text(state, &schedule(), &limits()));
    }
    for path in ["/", "/home", "/root", "/srv", "/var/log", "/tmp", "/mnt/x"] {
        assert_plain("offer", &protect_offer(path).replace(".snapshots", ""));
        assert_plain("detail", &area_detail(&describe(path)));
        assert_plain("protected", &protected_text(path));
        assert_plain(
            "line",
            &area_line(&Area {
                mount_point: path.into(),
                protected_by: None,
            }),
        );
    }
}

#[test]
fn the_short_offer_keeps_what_matters_and_is_short_enough_for_a_small_window() {
    let offer = protect_summary("/home");
    for must in [
        "Protect Home (/home)?",
        "your own files",
        "Recommended:",
        "never on a schedule",
        "does not copy, move or change any of your files",
        "takes no snapshot yet",
    ] {
        assert!(offer.contains(must), "`{must}` missing:\n{offer}");
    }
    assert!(
        offer.lines().count() <= 13,
        "{} lines:\n{offer}",
        offer.lines().count()
    );
    assert_plain("short offer", &offer.replace(".snapshots", ""));
}

#[test]
fn a_failure_says_what_went_wrong_and_that_nothing_changed() {
    let error = SetupError::Failed(crate::ports::snapshots::BackendError::NotSupported(
        "path is not usable".into(),
    ));
    let text = failure_text("Home", &error);
    assert!(
        text.contains("Could not protect Home")
            && text.contains("path is not usable")
            && text.contains("Nothing was changed"),
        "{text}"
    );
}

#[test]
fn the_offer_says_what_will_happen_and_what_will_not() {
    let offer = protect_offer("/home");
    for must in [
        "Protect Home (/home)?",
        "your own files",
        "could not be undone",
        "What snapall will do",
        "never on a schedule",
        "does not copy, move or change any of your files",
        "takes no snapshot yet",
        "grows only as files change",
    ] {
        assert!(offer.contains(must), "`{must}` missing:\n{offer}");
    }
}

#[test]
fn an_area_that_is_not_worth_it_is_not_presented_as_a_reason_to_protect_it() {
    let offer = protect_offer("/var/log");
    assert!(
        offer.contains("Not recommended:") && !offer.contains("Recommended:\n"),
        "{offer}"
    );
    assert!(offer.contains("mostly waste space"), "{offer}");
}

#[test]
fn automatic_cleanup_is_read_from_what_the_system_says_and_never_guessed() {
    assert_eq!(
        cleanup_from(Some("enabled\n"), Some("active\n")),
        Cleanup::On
    );
    assert_eq!(
        cleanup_from(Some("disabled"), Some("inactive")),
        Cleanup::Off
    );
    // enabled but not running is not "on": nothing is cleaning anything up
    assert_eq!(
        cleanup_from(Some("enabled"), Some("inactive")),
        Cleanup::Unavailable
    );
    assert_eq!(
        cleanup_from(Some("enabled"), Some("failed")),
        Cleanup::Unavailable
    );
    assert_eq!(cleanup_from(Some("enabled"), None), Cleanup::Unavailable);
    for other in [Some("masked"), Some("not-found"), Some(""), None] {
        assert_eq!(
            cleanup_from(other, Some("active")),
            Cleanup::Unavailable,
            "{other:?}"
        );
    }
    assert!(cleanup_text(Cleanup::Off, &[], &[]).contains("pile up"));
    assert!(
        cleanup_text(Cleanup::Unavailable, &[], &[]).contains("could not find out"),
        "an unknown state is not presented as a known one"
    );
}

#[test]
fn a_line_says_plainly_whether_an_area_is_protected() {
    let home = Area {
        mount_point: "/home".into(),
        protected_by: None,
    };
    assert_eq!(area_line(&home), "Home (/home): NOT protected");
    let system = Area {
        mount_point: "/".into(),
        protected_by: Some(crate::vocabulary::domain::AreaName::parse("root").unwrap()),
    };
    assert_eq!(area_line(&system), "System (/): protected");
}

#[test]
fn the_cleanup_text_is_made_from_the_real_schedule_and_limits_and_changes_when_they_do() {
    let text = cleanup_text(Cleanup::On, &schedule(), &limits());
    assert!(
        text.contains(
            "10 minutes after the computer starts, then again 1 day after each run starts"
        ),
        "{text}"
    );
    assert!(
        text.contains("Home keeps the newest 50 and never removes one younger than 30 minutes"),
        "{text}"
    );
    assert!(text.contains("System keeps the newest 20"), "{text}");
    // the same text with other facts says other things: nothing is typed in
    let other = vec![Limit {
        area: "Home".into(),
        keep: Some(7),
        min_age_seconds: Some(7200),
    }];
    let text = cleanup_text(Cleanup::On, &["every hour".into()], &other);
    assert!(
        text.contains("runs every hour")
            && text.contains("keeps the newest 7")
            && text.contains("2 hours"),
        "{text}"
    );
    assert!(
        !text.contains("50") && !text.contains("30 minutes") && !text.contains("once a day"),
        "{text}"
    );
}

#[test]
fn when_nothing_is_known_nothing_is_made_up() {
    let text = cleanup_text(Cleanup::On, &[], &[]);
    assert!(
        text.contains("on the schedule your system gives it"),
        "{text}"
    );
    assert!(
        !text.contains("keeps the newest"),
        "no limit is claimed: {text}"
    );
    let offer = cleanup_offer(&[], &[]);
    assert!(
        offer.contains("on the schedule your system gives it") && !offer.contains("keeps"),
        "{offer}"
    );
    // a limit with no age says only what it knows
    let some = cleanup_text(
        Cleanup::Off,
        &[],
        &[Limit {
            area: "Home".into(),
            keep: Some(3),
            min_age_seconds: None,
        }],
    );
    assert!(
        some.contains("Home keeps the newest 3.") && !some.contains("younger"),
        "{some}"
    );
}

#[test]
fn durations_are_said_in_the_largest_whole_unit() {
    for (seconds, words) in [
        (0, "0 seconds"),
        (1, "1 second"),
        (90, "90 seconds"),
        (1800, "30 minutes"),
        (3600, "1 hour"),
        (7200, "2 hours"),
        (86_400, "1 day"),
        (172_800, "2 days"),
    ] {
        assert_eq!(seconds_words(seconds), words);
    }
}

#[test]
fn room_counting_is_explained_with_what_it_gives_what_it_costs_and_when_to_leave_it_off() {
    let off = room_counting_text(QuotaMode::Off);
    for must in [
        "not counting how much room each snapshot holds",
        "Switching counting on",
        "The price",
        "slow some operations down",
        "deleting snapshots most of all",
        "whole disk",
        "switched off again",
        "leaving it off is the better choice",
    ] {
        assert!(off.contains(must), "`{must}` missing: {off}");
    }
    let full = room_counting_text(QuotaMode::Full);
    assert!(
        full.contains("what each snapshot alone holds")
            && full.contains("The price")
            && full.contains("Switching it off"),
        "{full}"
    );
    let simple = room_counting_text(QuotaMode::Simple);
    assert!(
        simple.contains("cannot be told from it")
            && simple.contains("not something snapall can do in place"),
        "{simple}"
    );
    assert!(room_counting_text(QuotaMode::Unknown).contains("could not find out"));
    for mode in [
        QuotaMode::Off,
        QuotaMode::Full,
        QuotaMode::Simple,
        QuotaMode::Unknown,
    ] {
        assert_plain("room counting", &room_counting_text(mode));
    }
}
