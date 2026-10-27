// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! What the window asks before it changes anything: one question, at the weight the core's table gives the
//! change, never answered by a bare Enter, and each change leaves its way back in the record.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{
    plan_snapshots, run_plan, Act, ConfigName, Description, JournalEvent, Rollback,
    SnapshotBackend, SystemProbe, Weight,
};
use snapall_tui::model::{Button, Click, Key, Modal, Msg};
use snapall_tui::{view, Session, UiContext};

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

fn flat(session: &Session<'_>) -> String {
    let mut terminal = Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height)).unwrap();
    terminal
        .draw(|frame| view::render(&session.state, frame))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..SCREEN.height)
        .map(|y| {
            (0..SCREEN.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .map(|c| {
            if "│┌┐└┘╭╮╰╯─┃┏┓┗┛━".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn home() -> ConfigName {
    ConfigName::parse("home").unwrap()
}

/// One area with a file that was "v1" when its snapshots were taken and is "v2" now.
fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["home"]);
    backend.write_file("home", "/mnt/home/n.txt", "v1");
    let plan = plan_snapshots(&backend, None).unwrap();
    for note in ["one", "two"] {
        run_plan(&backend, &plan, &Description::parse(note).unwrap(), None);
    }
    backend.write_file("home", "/mnt/home/n.txt", "v2");
    backend
}

fn system() -> FakeSystem {
    FakeSystem::new().at("2026-10-25 12:00:00")
}

fn admin() -> UiContext {
    UiContext {
        administrator: true,
        ..UiContext::default()
    }
}

fn type_text(session: &mut Session<'_>, text: &str) {
    for letter in text.chars() {
        session.press(Key::Char(letter));
    }
}

fn clear(session: &mut Session<'_>) {
    for _ in 0..40 {
        session.press(Key::Backspace);
    }
}

/// The question that is open.
fn question(session: &Session<'_>) -> Option<(Act, Option<String>)> {
    match &session.state.modal {
        Some(Modal::Confirm(q)) => Some((q.act, q.target.clone())),
        _ => None,
    }
}

fn snapshots(backend: &InMemoryBackend) -> usize {
    backend.list_snapshots(&home()).unwrap().len()
}

#[test]
fn deleting_asks_for_the_word_destroy_and_y_or_enter_is_never_enough() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Right);
    session.press(Key::Char('d'));
    session.press(Key::Char('y')); // the old way to say yes to the first prompt: it only opens the question
    assert_eq!(question(&session), Some((Act::DeleteSnapshots, None)));
    assert_eq!(Act::DeleteSnapshots.weight(), Weight::Destroys);
    let text = flat(&session);
    assert!(
        text.contains("Type destroy, then Enter, to go ahead"),
        "{text}"
    );
    assert!(
        text.contains("cannot be brought back"),
        "the loss is said where it is answered: {text}"
    );
    for wrong in ["", "y", "yes", "Destroy", "destroy now", "delete"] {
        clear(&mut session);
        type_text(&mut session, wrong);
        session.press(Key::Enter);
        assert!(question(&session).is_some(), "{wrong:?} is not the answer");
        assert_eq!(snapshots(&backend), 2, "{wrong:?} deleted nothing");
    }
    clear(&mut session);
    type_text(&mut session, "destroy");
    session.press(Key::Enter);
    assert!(session.state.modal.is_none());
    assert_eq!(snapshots(&backend), 1, "the word deletes it");
    let events = system.events();
    assert!(
        events.iter().any(|e| matches!(e, JournalEvent::Rollback { rollback: Rollback::Advice(text), .. } if text.contains("cannot be brought back"))),
        "it is recorded as what cannot be undone, with what protects next time: {events:?}"
    );
}

#[test]
fn a_restore_asks_for_y_or_yes_and_its_way_back_names_the_snapshot_it_took_just_before() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Right);
    session.press(Key::Char('u'));
    session.press(Key::Enter);
    assert_eq!(question(&session), Some((Act::Restore, None)));
    assert!(flat(&session).contains("Type y or yes, then Enter, to go ahead"));
    for wrong in ["", "yep", "no", "destroy"] {
        clear(&mut session);
        type_text(&mut session, wrong);
        session.press(Key::Enter);
        assert!(question(&session).is_some(), "{wrong:?} is not the answer");
        assert_eq!(
            backend.read_file("home", "/mnt/home/n.txt").as_deref(),
            Some("v2"),
            "{wrong:?} restored nothing"
        );
    }
    clear(&mut session);
    type_text(&mut session, "YES");
    session.press(Key::Enter);
    assert_eq!(
        backend.read_file("home", "/mnt/home/n.txt").as_deref(),
        Some("v1"),
        "it is put back"
    );
    let safety = backend.list_snapshots(&home()).unwrap().last().unwrap().id;
    let way_back = system
        .events()
        .into_iter()
        .find_map(|e| match e {
            JournalEvent::Rollback {
                rollback: Rollback::Undo(undo),
                ..
            } => Some(undo),
            _ => None,
        })
        .expect("a way back was recorded");
    assert_eq!(
        way_back.commands,
        [vec![
            "restore".to_owned(),
            "home".to_owned(),
            safety.to_string()
        ]]
    );
    assert!(
        way_back
            .what
            .contains(&format!("just before: snapshot #{safety}")),
        "{}",
        way_back.what
    );
    assert_eq!(way_back.weight, Weight::Changes);
}

#[test]
fn taking_a_snapshot_never_asks_and_records_that_undoing_it_is_a_deletion() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('c'));
    session.press(Key::Enter);
    assert!(
        session.state.modal.is_none(),
        "no question: it is the safety net"
    );
    assert_eq!(snapshots(&backend), 3);
    let way_back = system.events().into_iter().find_map(|e| match e {
        JournalEvent::Rollback {
            rollback: Rollback::Undo(undo),
            ..
        } => Some(undo),
        _ => None,
    });
    let way_back = way_back.expect("taking a snapshot records its way back");
    assert_eq!(
        way_back.weight,
        Weight::Destroys,
        "undoing it is deleting it"
    );
    assert_eq!(way_back.commands[0][0], "delete");
}

#[test]
fn escape_changes_nothing_and_says_so() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Right);
    session.press(Key::Char('d'));
    session.press(Key::Enter);
    type_text(&mut session, "destroy");
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
    assert_eq!(snapshots(&backend), 2);
    assert!(format!("{:?}", session.state.notice).contains("Nothing was changed"));
}

#[test]
fn the_button_on_the_question_does_nothing_until_the_answer_is_typed() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Right);
    session.press(Key::Char('d'));
    session.press(Key::Enter);
    session.dispatch(Msg::Click(Click::Button(Button::Confirm)));
    assert!(question(&session).is_some(), "the button is not an answer");
    assert_eq!(snapshots(&backend), 2);
    type_text(&mut session, "destroy");
    session.dispatch(Msg::Click(Click::Button(Button::Confirm)));
    assert_eq!(
        snapshots(&backend),
        1,
        "with the answer typed it works by mouse too"
    );
}

#[test]
fn every_change_that_is_not_a_loss_asks_for_y_or_yes_each_by_the_tables_weight() {
    for act in [
        Act::Restore,
        Act::RestorePoint,
        Act::Protect,
        Act::RoomCounting,
        Act::ChangeGroups,
        Act::ControlJob,
        Act::StopChange,
    ] {
        assert_eq!(act.weight(), Weight::Changes, "{act:?}");
    }
    // and a few of them, in the window, open the question at that weight
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('g'));
    session.press(Key::Char('n'));
    type_text(&mut session, "games");
    session.press(Key::Enter);
    session.press(Key::Char(' '));
    session.press(Key::Enter);
    assert_eq!(
        question(&session),
        Some((Act::ChangeGroups, None)),
        "saving a group asks"
    );
    assert!(
        system.groups().unwrap().is_empty(),
        "and nothing is saved until it is answered"
    );
    session.press(Key::Esc);
    assert!(
        matches!(session.state.modal, Some(Modal::Groups(_))),
        "cancelling goes back to the groups, not out of them"
    );
    session.press(Key::Char('n'));
    type_text(&mut session, "games");
    session.press(Key::Enter);
    session.press(Key::Char(' '));
    session.press(Key::Enter);
    type_text(&mut session, "y");
    session.press(Key::Enter);
    assert!(
        matches!(session.state.modal, Some(Modal::Groups(_))),
        "answered, it is back at the groups"
    );
    assert_eq!(
        system.groups().unwrap().names().count(),
        1,
        "and the group is kept"
    );
    let undo = system.events().into_iter().find_map(|e| match e {
        JournalEvent::Rollback {
            rollback: Rollback::Undo(undo),
            ..
        } => Some(undo),
        _ => None,
    });
    assert_eq!(
        undo.unwrap().commands,
        [["groups", "remove", "games"]],
        "a new group is undone by taking it away"
    );
}

#[test]
fn the_question_fits_eighty_by_twenty_four_and_leaves_the_bars_and_uses_no_untold_word() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Right);
    session.press(Key::Char('d'));
    session.press(Key::Enter);
    let text = flat(&session);
    for must in [
        "Delete for good",
        "Delete 1 snapshot(s) of home for good",
        "Go ahead",
    ] {
        assert!(text.contains(must), "`{must}` missing:\n{text}");
    }
    for jargon in ["btrfs", "subvolume", "snapper", "config"] {
        assert!(
            !text.to_lowercase().contains(jargon),
            "`{jargon}` in:\n{text}"
        );
    }
    assert!(
        text.contains("q quit") && text.contains("snapshot"),
        "the bars are not covered:\n{text}"
    );
}

#[test]
fn the_activity_tab_shows_how_to_get_back_from_a_change_or_that_there_is_no_way() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    // a restore can be undone
    session.press(Key::Right);
    session.press(Key::Char('u'));
    session.press(Key::Enter);
    session.confirm();
    session.press(Key::Char('3'));
    let shown = flat(&session);
    assert!(
        shown.contains("Way back: Put home back to how it was just before: snapshot #"),
        "{shown}"
    );
    assert!(shown.contains("To do it: snapall undo "), "{shown}");
    // a deletion cannot
    session.press(Key::Char('1'));
    session.press(Key::Right);
    session.press(Key::Char('d'));
    session.press(Key::Enter);
    session.confirm();
    session.press(Key::Char('3'));
    let shown = flat(&session);
    assert!(
        shown.contains("It cannot be undone. A deleted snapshot cannot be brought back"),
        "{shown}"
    );
}
