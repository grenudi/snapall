// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The setup conversation, with a scripted person.

#![cfg(test)]

use snapall_core::vocabulary::explain::Cleanup;
use snapall_core::{AreaName, SnapshotBackend};
use std::error::Error;

use super::*;
use snapall_core::ports::testing::InMemoryBackend;

/// A person who answers from a script and writes down everything said to them.
struct Person {
    answers: Vec<bool>,
    asked: Vec<String>,
    said: String,
    cleanup_switched_on: bool,
    cleanup_fails: bool,
}

impl Person {
    fn answering(answers: &[bool]) -> Self {
        Self {
            answers: answers.to_vec(),
            asked: Vec::new(),
            said: String::new(),
            cleanup_switched_on: false,
            cleanup_fails: false,
        }
    }
}

impl Talk for Person {
    fn say(&mut self, text: &str) {
        self.said.push_str(text);
        self.said.push('\n');
    }
    fn ask(&mut self, question: &str) -> Result<bool, Box<dyn Error>> {
        self.asked.push(question.to_owned());
        Ok(if self.answers.is_empty() {
            false
        } else {
            self.answers.remove(0)
        })
    }
    fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>> {
        if self.cleanup_fails {
            return Err("Failed to enable unit".into());
        }
        self.cleanup_switched_on = true;
        Ok(())
    }
}

const MOUNTINFO: &str = "\\
24 1 0:21 /@ / rw - btrfs /dev/nvme0n1p2 rw,subvol=/@\n\\
25 24 0:21 /@home /home rw - btrfs /dev/nvme0n1p2 rw,subvol=/@home\n\\
26 24 0:21 /@log /var/log rw - btrfs /dev/nvme0n1p2 rw,subvol=/@log\n";

fn facts(is_root: bool, cleanup: Cleanup) -> Facts {
    Facts {
        mountinfo: MOUNTINFO.into(),
        is_root,
        cleanup,
        cleanup_schedule: Vec::new(),
    }
}
fn options() -> Options {
    Options {
        area: None,
        yes: false,
        dry_run: false,
    }
}
/// A machine whose only protected area is the system.
fn machine() -> InMemoryBackend {
    InMemoryBackend::with_areas(&["root"]).with_root_area("root")
}

#[test]
fn it_explains_first_lists_the_areas_and_offers_only_what_is_worth_protecting() {
    let backend = machine();
    let mut person = Person::answering(&[false]);
    let report = run(&backend, &facts(true, Cleanup::On), &mut person, &options()).unwrap();
    for must in [
        "A snapshot is a saved moment",
        "System (/): protected",
        "Home (/home): NOT protected",
        "Protect Home (/home)?",
        "Skipped.",
    ] {
        assert!(
            person.said.contains(must),
            "`{must}` not said:\n{}",
            person.said
        );
    }
    assert!(
        !person.said.contains("Protect System"),
        "the system is already protected"
    );
    assert!(
        !person.said.contains("Protect System logs"),
        "logs are not worth offering"
    );
    assert_eq!(person.asked, ["Protect Home now?"]);
    assert_eq!(report.declined, ["/home"]);
    assert_eq!(
        backend.list_areas().unwrap().len(),
        1,
        "a no changes nothing"
    );
}

#[test]
fn a_yes_protects_the_area_turns_off_schedules_and_says_what_comes_next() {
    let backend = machine();
    let mut person = Person::answering(&[true]);
    let report = run(&backend, &facts(true, Cleanup::On), &mut person, &options()).unwrap();
    assert_eq!(report.protected_now.len(), 1);
    assert_eq!(backend.timeline_create("home"), Some(false));
    assert!(
        person.said.contains("Home (/home) is protected now"),
        "{}",
        person.said
    );
    assert!(
        person
            .said
            .contains("Take the first snapshot of everything with `sudo snapall`"),
        "{}",
        person.said
    );
    assert!(report.succeeded());
}

#[test]
fn yes_accepts_the_recommendations_without_a_single_question() {
    let backend = machine();
    let mut person = Person::answering(&[]);
    let report = run(
        &backend,
        &facts(true, Cleanup::Off),
        &mut person,
        &Options {
            yes: true,
            ..options()
        },
    )
    .unwrap();
    assert!(person.asked.is_empty());
    assert_eq!(report.protected_now.len(), 1);
    assert!(
        person.cleanup_switched_on && report.cleanup_switched_on,
        "cleanup was off, so it was switched on"
    );
}

#[test]
fn a_dry_run_explains_everything_and_changes_nothing_not_even_without_administrator_rights() {
    let backend = machine();
    let mut person = Person::answering(&[]);
    let report = run(
        &backend,
        &facts(false, Cleanup::Off),
        &mut person,
        &Options {
            dry_run: true,
            ..options()
        },
    )
    .unwrap();
    assert!(person.asked.is_empty() && !person.cleanup_switched_on);
    assert_eq!(backend.list_areas().unwrap().len(), 1);
    assert!(
        person.said.contains("Protect Home (/home)?")
            && person.said.contains("Dry run: nothing was changed"),
        "{}",
        person.said
    );
    assert!(
        person.said.contains("Run `sudo snapall setup` to do this"),
        "{}",
        person.said
    );
    assert!(report.dry_run && report.protected_now.is_empty());
}

#[test]
fn without_administrator_rights_it_says_so_in_snapalls_own_terms_before_saying_anything_else() {
    let backend = machine();
    let mut person = Person::answering(&[]);
    let error = run(
        &backend,
        &facts(false, Cleanup::On),
        &mut person,
        &options(),
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("sudo snapall setup"), "{error}");
    assert!(person.said.is_empty(), "nothing was said before refusing");
}

#[test]
fn automatic_cleanup_is_explained_and_only_offered_when_it_is_off() {
    let backend = machine();
    for (state, offered) in [
        (Cleanup::On, false),
        (Cleanup::Off, true),
        (Cleanup::Unavailable, false),
    ] {
        let mut person = Person::answering(&[false, false]);
        run(&backend, &facts(true, state), &mut person, &options()).unwrap();
        assert_eq!(person.said.contains("Switch it on?"), offered, "{state:?}");
        assert_eq!(
            person.asked.iter().any(|q| q.contains("cleanup")),
            offered,
            "{state:?}"
        );
    }
    // and when the person declines it, it stays off
    let mut person = Person::answering(&[false, false]);
    let report = run(
        &backend,
        &facts(true, Cleanup::Off),
        &mut person,
        &options(),
    )
    .unwrap();
    assert!(!person.cleanup_switched_on && !report.cleanup_switched_on);
}

#[test]
fn a_failure_is_explained_plainly_nothing_is_left_changed_and_the_report_says_it_failed() {
    let backend = machine();
    backend.refuse_option_changes();
    let mut person = Person::answering(&[true]);
    let report = run(&backend, &facts(true, Cleanup::On), &mut person, &options()).unwrap();
    assert!(!report.succeeded());
    assert!(
        person.said.contains("Could not protect Home")
            && person.said.contains("Nothing was changed"),
        "{}",
        person.said
    );
    assert_eq!(
        backend.list_areas().unwrap().len(),
        1,
        "the half-made area was taken back"
    );
    // a cleanup that cannot be switched on is a failure too, said plainly
    let backend = machine();
    let mut person = Person::answering(&[false, true]);
    person.cleanup_fails = true;
    let report = run(
        &backend,
        &facts(true, Cleanup::Off),
        &mut person,
        &options(),
    )
    .unwrap();
    assert!(
        person
            .said
            .contains("Could not switch automatic cleanup on")
            && !report.succeeded(),
        "{}",
        person.said
    );
}

#[test]
fn a_named_area_is_offered_even_if_optional_and_one_that_does_not_exist_is_refused_listing_the_real_ones(
) {
    let backend = machine();
    let mut person = Person::answering(&[true]);
    let report = run(
        &backend,
        &facts(true, Cleanup::On),
        &mut person,
        &Options {
            area: Some("/var/log".into()),
            ..options()
        },
    )
    .unwrap();
    assert!(person.said.contains("Not recommended:"), "{}", person.said);
    assert_eq!(report.protected_now[0].0, "/var/log");
    let error = run(
        &backend,
        &facts(true, Cleanup::On),
        &mut Person::answering(&[]),
        &Options {
            area: Some("/nowhere".into()),
            ..options()
        },
    )
    .err()
    .unwrap();
    assert!(
        error.to_string().contains("not a part of the disk")
            && error.to_string().contains("/, /home, /var/log"),
        "{error}"
    );
}

#[test]
fn when_everything_worth_protecting_is_protected_it_says_so_and_asks_nothing() {
    let backend = InMemoryBackend::with_areas(&["root", "home"]).with_root_area("root");
    // make the in-memory `home` area's subvolume /home, as a real one would be
    let mut person = Person::answering(&[]);
    let facts = Facts {
        mountinfo: MOUNTINFO.into(),
        is_root: true,
        cleanup: Cleanup::On,
        cleanup_schedule: Vec::new(),
    };
    let both = InMemoryBackend::with_areas(&[]);
    both.create_area(&AreaName::parse("root").unwrap(), "/")
        .unwrap();
    both.create_area(&AreaName::parse("home").unwrap(), "/home")
        .unwrap();
    let report = run(&both, &facts, &mut person, &options()).unwrap();
    assert!(
        person.asked.is_empty() && report.protected_now.is_empty(),
        "{}",
        person.said
    );
    assert!(!person.said.contains("Protect Home"), "{}", person.said);
    let _ = backend;
}

#[test]
fn when_there_is_nothing_to_set_up_it_says_so_instead_of_staying_silent() {
    let facts = Facts {
        mountinfo: MOUNTINFO.into(),
        is_root: true,
        cleanup: Cleanup::On,
        cleanup_schedule: Vec::new(),
    };
    let both = InMemoryBackend::with_areas(&[]);
    both.create_area(&AreaName::parse("root").unwrap(), "/")
        .unwrap();
    both.create_area(&AreaName::parse("home").unwrap(), "/home")
        .unwrap();
    let mut person = Person::answering(&[]);
    run(&both, &facts, &mut person, &options()).unwrap();
    assert!(
        person
            .said
            .contains("protected already, so there is nothing to set up"),
        "{}",
        person.said
    );
    let mut person = Person::answering(&[]);
    run(
        &both,
        &facts,
        &mut person,
        &Options {
            area: Some("/home".into()),
            ..options()
        },
    )
    .unwrap();
    assert!(
        person
            .said
            .contains("Home (/home) is already protected, so there is nothing to set up"),
        "{}",
        person.said
    );
}

#[test]
fn the_window_is_offered_what_is_worth_protecting_best_first_and_a_hint_only_for_the_recommended() {
    let backend = machine();
    let offered = offers(&backend, &facts(true, Cleanup::On));
    let titles: Vec<&str> = offered.iter().map(|o| o.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Home"],
        "the logs are not worth offering and the system is protected"
    );
    assert!(offered[0].recommended && offered[0].summary.contains("Protect Home (/home)?"));
    let hint = hint_for(&offered).unwrap();
    assert_eq!(
        hint,
        "Home has no snapshots yet. Press p to see what that means and protect it."
    );
    assert_eq!(hint_for(&[]), None);
    let optional = snapall_tui::Offer {
        mount_point: "/srv".into(),
        title: "Served data".into(),
        summary: String::new(),
        recommended: false,
    };
    assert_eq!(
        hint_for(&[optional]),
        None,
        "nothing recommended is missing, so no nagging"
    );
}
