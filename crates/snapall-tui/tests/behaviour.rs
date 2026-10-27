// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The UI's behaviour, driven the way a user drives it: key presses against an in-memory
//! backend. No terminal is involved, so every flow is a plain test.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use snapall_core::testing::InMemoryBackend;
use snapall_core::{ConfigName, Description, SnapshotBackend};
use snapall_tui::model::{Key, Load, Modal, Notice, Pane};
use snapall_tui::Session;

fn name(s: &str) -> ConfigName {
    ConfigName::parse(s).unwrap()
}

fn seed(backend: &InMemoryBackend, config: &str, descriptions: &[&str]) {
    for d in descriptions {
        backend
            .create_snapshot(&name(config), &Description::parse(d).unwrap())
            .unwrap();
    }
}

fn type_text(session: &mut Session<'_>, text: &str) {
    for c in text.chars() {
        session.press(Key::Char(c));
    }
}

fn press(session: &mut Session<'_>, keys: &[Key]) {
    for key in keys {
        session.press(*key);
    }
}

fn descriptions_of(session: &Session<'_>) -> Vec<String> {
    session
        .state
        .displayed_snapshots()
        .iter()
        .map(|s| s.description.clone())
        .collect()
}

#[test]
fn startup_loads_the_configs_and_only_the_selected_ones_snapshots() {
    let backend = InMemoryBackend::with_configs(&["root", "home"]);
    seed(&backend, "home", &["a", "b"]);
    let session = Session::new(&backend);

    let names: Vec<_> = session
        .state
        .entries()
        .iter()
        .map(|e| e.name.to_string())
        .collect();
    assert_eq!(names, ["home", "root"], "sorted by name");
    assert_eq!(session.state.config_cursor, 0);
    assert!(matches!(
        session.state.entries()[0].snapshots,
        Load::Loaded(_)
    ));
    assert_eq!(session.state.entries()[1].snapshots, Load::NotLoaded);
    assert_eq!(descriptions_of(&session), ["b", "a"], "newest first");
}

#[test]
fn moving_to_another_config_loads_its_snapshots_lazily() {
    let backend = InMemoryBackend::with_configs(&["root", "home"]);
    seed(&backend, "root", &["r1"]);
    let mut session = Session::new(&backend);

    session.press(Key::Down);
    assert_eq!(
        session.state.selected_config().unwrap().name.as_str(),
        "root"
    );
    assert_eq!(descriptions_of(&session), ["r1"]);

    // The cursor does not run off the end.
    session.press(Key::Down);
    assert_eq!(session.state.config_cursor, 1);
}

#[test]
fn create_asks_for_a_description_then_creates_and_shows_the_new_snapshot_first() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    seed(&backend, "home", &["old"]);
    let mut session = Session::new(&backend);

    session.press(Key::Char('c'));
    assert!(matches!(session.state.modal, Some(Modal::Describe { .. })));
    type_text(&mut session, "before theming");
    session.press(Key::Enter);

    assert!(session.state.modal.is_none());
    assert_eq!(descriptions_of(&session), ["before theming", "old"]);
    assert_eq!(session.state.snapshot_cursor, 0);
    assert!(matches!(&session.state.notice, Some(Notice::Info(t)) if t.contains("home #2")));
}

#[test]
fn an_empty_description_uses_the_default_and_a_snapshots_every_config() {
    let backend = InMemoryBackend::with_configs(&["root", "home"]);
    let mut session = Session::new(&backend);

    press(&mut session, &[Key::Char('a'), Key::Enter]);

    let taken = backend.snapshots();
    assert_eq!(taken.len(), 2);
    assert!(taken.iter().all(|(_, _, d)| d == "snapall"));
    assert!(
        matches!(&session.state.notice, Some(Notice::Info(t)) if t.contains("home #1") && t.contains("root #1"))
    );
}

#[test]
fn escape_cancels_without_touching_anything_and_q_is_text_inside_the_prompt() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    let mut session = Session::new(&backend);

    session.press(Key::Char('c'));
    session.press(Key::Char('q'));
    assert!(
        !session.state.quit,
        "q must be typed, not quit, while a prompt is open"
    );
    session.press(Key::Esc);

    assert!(session.state.modal.is_none());
    assert!(backend.snapshots().is_empty());
}

#[test]
fn backspace_edits_and_the_description_is_length_limited() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    let mut session = Session::new(&backend);
    session.press(Key::Char('c'));
    type_text(&mut session, "abc");
    session.press(Key::Backspace);
    type_text(&mut session, &"x".repeat(Description::MAX_LEN + 50));
    let Some(Modal::Describe { input, .. }) = &session.state.modal else {
        panic!("prompt should be open");
    };
    assert!(input.starts_with("abx"));
    assert_eq!(input.chars().count(), Description::MAX_LEN);
}

#[test]
fn marking_moves_on_and_delete_removes_exactly_the_marked_snapshots() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    seed(&backend, "home", &["one", "two", "three", "four"]);
    let mut session = Session::new(&backend);

    // newest first: four, three, two, one. Mark "four" and "two".
    press(
        &mut session,
        &[Key::Right, Key::Char(' '), Key::Down, Key::Char(' ')],
    );
    assert_eq!(session.state.marked.len(), 2);

    session.press(Key::Char('d'));
    assert!(
        matches!(session.state.modal, Some(Modal::ConfirmDelete { ref ids, .. }) if ids.len() == 2)
    );
    session.press(Key::Char('y'));
    session.confirm();

    assert_eq!(descriptions_of(&session), ["three", "one"]);
    assert!(session.state.marked.is_empty());
    assert!(matches!(&session.state.notice, Some(Notice::Info(t)) if t.contains("deleted")));
}

#[test]
fn delete_without_marks_targets_the_row_under_the_cursor_and_can_be_cancelled() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    seed(&backend, "home", &["one", "two"]);
    let mut session = Session::new(&backend);

    press(&mut session, &[Key::Right, Key::Char('d'), Key::Char('n')]);
    assert_eq!(backend.snapshots().len(), 2, "cancel deletes nothing");

    press(&mut session, &[Key::Char('d'), Key::Enter]);
    session.confirm();
    assert_eq!(
        descriptions_of(&session),
        ["one"],
        "the newest (under the cursor) went"
    );
}

#[test]
fn delete_is_not_offered_from_the_configs_pane() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    seed(&backend, "home", &["one"]);
    let mut session = Session::new(&backend);
    assert_eq!(session.state.focus, Pane::Configs);

    session.press(Key::Char('d'));
    assert!(session.state.modal.is_none());
    assert!(matches!(session.state.notice, Some(Notice::Info(_))));
    assert_eq!(backend.snapshots().len(), 1);
}

#[test]
fn a_failing_config_is_reported_while_the_others_still_get_their_snapshot() {
    let backend = InMemoryBackend::with_configs(&["home", "root"]).failing_on("root");
    let mut session = Session::new(&backend);

    press(&mut session, &[Key::Char('a'), Key::Enter]);

    assert_eq!(backend.snapshots().len(), 1);
    assert!(
        matches!(&session.state.notice, Some(Notice::Error(t)) if t.contains("home #1") && t.contains("root")),
        "{:?}",
        session.state.notice
    );
}

#[test]
fn no_configs_is_explained_not_empty() {
    let backend = InMemoryBackend::with_configs(&[]);
    let mut session = Session::new(&backend);

    assert!(session.state.entries().is_empty());
    assert!(
        matches!(&session.state.notice, Some(Notice::Error(t)) if t.contains("no protected areas") && !t.contains("create-config"))
    );
    press(&mut session, &[Key::Char('c'), Key::Char('a')]);
    assert!(
        session.state.modal.is_none(),
        "nothing to snapshot, so no prompt"
    );
}

#[test]
fn reload_keeps_the_selection_by_name() {
    let backend = InMemoryBackend::with_configs(&["home", "root"]);
    let mut session = Session::new(&backend);
    session.press(Key::Down);
    session.press(Key::Char('r'));
    assert_eq!(
        session.state.selected_config().unwrap().name.as_str(),
        "root"
    );
}

#[test]
fn quit_keys_quit_and_help_opens_turns_its_pages_and_closes_with_escape() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    let mut session = Session::new(&backend);
    press(&mut session, &[Key::Char('?')]);
    assert_eq!(session.state.modal, Some(Modal::Help { page: 0 }));
    // an unrelated key does nothing: help is two pages now, so it no longer closes on any key
    session.press(Key::Char('x'));
    assert_eq!(session.state.modal, Some(Modal::Help { page: 0 }));
    // Enter, Tab and the arrows turn the page, and turn back round after the last
    session.press(Key::Enter);
    assert_eq!(session.state.modal, Some(Modal::Help { page: 1 }));
    session.press(Key::Tab);
    assert_eq!(session.state.modal, Some(Modal::Help { page: 0 }));
    session.press(Key::Right);
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
    assert!(!session.state.quit, "closing the help does not quit");

    session.press(Key::Char('q'));
    assert!(session.state.quit);

    let mut second = Session::new(&backend);
    second.press(Key::CtrlC);
    assert!(second.state.quit);
}
