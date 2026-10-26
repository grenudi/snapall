// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Changing how many snapshots an area keeps, from the Health tab: what it shows, what it would remove, the
//! second Enter that a destructive change needs, and what snapper ends up holding.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{plan_snapshots, run_plan, ConfigName, Description, SnapshotBackend};
use snapall_tui::model::{Key, Modal};
use snapall_tui::tabs::health::RowAction;
use snapall_tui::{view, Session, UiContext};

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

fn rows(session: &Session<'_>) -> Vec<String> {
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
        .collect()
}

fn flat(session: &Session<'_>) -> String {
    rows(session)
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

/// One area with eight snapshots of the kind cleanup looks after.
fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["home"]);
    let plan = plan_snapshots(&backend, None).unwrap();
    for _ in 0..8 {
        run_plan(&backend, &plan, &Description::parse("one").unwrap(), None);
    }
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

/// The editor on the one area.
fn editing<'a>(
    backend: &'a InMemoryBackend,
    system: &'a FakeSystem,
    context: UiContext,
) -> Session<'a> {
    let mut session = Session::with_system(backend, system, context);
    session.press(Key::Char('2'));
    let at = session
        .state
        .health
        .rows()
        .iter()
        .position(|row| matches!(&row.action, RowAction::EditLimits(config) if *config == home()))
        .expect("the area's row leads to the editor");
    session.state.health.cursor = at;
    session.press(Key::Enter);
    assert!(matches!(session.state.modal, Some(Modal::Limits(_))));
    session
}

fn keep(backend: &InMemoryBackend) -> Option<u32> {
    backend.retention(&home()).unwrap().keep
}

#[test]
fn the_editor_shows_what_the_area_keeps_now_and_what_that_would_do() {
    let (backend, system) = (machine(), system());
    let session = editing(&backend, &system, admin());
    let text = flat(&session);
    for must in [
        "What Home keeps",
        "Snapshots you take yourself are among those it looks after",
        "Keep the newest [ 50 ] snapshots",
        "Never remove one younger than [ 30 ] minutes",
        "Cleanup of this area [ on ]",
        "Nothing would be removed: cleanup looks after 8 snapshots and the limit is 50",
    ] {
        assert!(text.contains(must), "`{must}` missing at 80x24:\n{text}");
    }
    let screen = rows(&session);
    assert!(
        screen[21].contains("snapshots") && screen[23].contains("q quit"),
        "the bars are not covered"
    );
}

#[test]
fn lowering_it_shows_what_would_go_and_applying_needs_a_second_enter() {
    let (backend, system) = (machine(), system());
    let mut session = editing(&backend, &system, admin());
    for _ in 0..45 {
        session.press(Key::Left); // 50 -> 5
    }
    let text = flat(&session);
    assert!(text.contains("[ 5 ]"), "{text}");
    assert!(
        text.contains(
            "3 of the 8 snapshots that cleanup looks after would be removed, the oldest first"
        ),
        "{text}"
    );
    assert!(text.contains("cannot be brought back"), "{text}");

    session.press(Key::Enter);
    assert!(
        matches!(session.state.modal, Some(Modal::Limits(_))),
        "the first Enter only asks"
    );
    assert!(
        flat(&session).contains("Press Enter again to apply this"),
        "{}",
        flat(&session)
    );
    assert_eq!(keep(&backend), Some(50), "nothing was changed yet");

    session.press(Key::Enter);
    assert!(session.state.modal.is_none());
    assert_eq!(keep(&backend), Some(5), "snapper now keeps five");
    assert!(format!("{:?}", session.state.notice).contains("takes effect at the next cleanup"));
    assert!(
        system
            .events()
            .iter()
            .any(|e| format!("{e:?}").contains("Changed how many snapshots home keeps")),
        "it is in the record"
    );
}

#[test]
fn changing_the_numbers_after_the_question_asks_again_because_the_answer_was_for_other_numbers() {
    let (backend, system) = (machine(), system());
    let mut session = editing(&backend, &system, admin());
    for _ in 0..45 {
        session.press(Key::Left);
    }
    session.press(Key::Enter); // asked
    session.press(Key::Left); // 5 -> 4: more would go
    session.press(Key::Enter);
    assert!(
        matches!(session.state.modal, Some(Modal::Limits(_))),
        "it asks again"
    );
    assert_eq!(keep(&backend), Some(50));
}

#[test]
fn a_change_that_removes_nothing_is_applied_at_once() {
    let (backend, system) = (machine(), system());
    let mut session = editing(&backend, &system, admin());
    for _ in 0..10 {
        session.press(Key::Left); // 50 -> 40: still far above the eight there are
    }
    session.press(Key::Enter);
    assert!(session.state.modal.is_none(), "nothing to be careful about");
    assert_eq!(keep(&backend), Some(40));
}

#[test]
fn the_age_under_which_nothing_goes_and_whether_cleanup_runs_are_set_the_same_way() {
    let (backend, system) = (machine(), system());
    let mut session = editing(&backend, &system, admin());
    session.press(Key::Down);
    session.press(Key::PageUp); // +10 minutes
    session.press(Key::Down);
    session.press(Key::Char(' ')); // cleanup off
    let text = flat(&session);
    assert!(
        text.contains("[ 40 ] minutes") && text.contains("[ off ]"),
        "{text}"
    );
    assert!(
        text.contains("Cleanup is switched off for this area, so nothing will be removed"),
        "{text}"
    );
    session.press(Key::Enter);
    let after = backend.retention(&home()).unwrap();
    assert_eq!(
        (after.min_age_seconds, after.cleanup, after.keep),
        (Some(2400), false, Some(50)),
        "only what was changed was changed"
    );
}

#[test]
fn it_cannot_be_set_to_keep_none_and_leaving_changes_nothing() {
    let (backend, system) = (machine(), system());
    let mut session = editing(&backend, &system, admin());
    for _ in 0..80 {
        session.press(Key::Left);
    }
    assert!(
        flat(&session).contains("[ 1 ]"),
        "it stops at one: keeping none would let cleanup remove everything"
    );
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
    assert_eq!(keep(&backend), Some(50), "Esc changes nothing");
}

#[test]
fn without_administrator_rights_it_says_so_and_changes_nothing() {
    let (backend, system) = (machine(), system());
    let mut session = editing(&backend, &system, UiContext::default());
    session.press(Key::Left);
    session.press(Key::Enter);
    assert!(format!("{:?}", session.state.notice).contains("needs administrator rights"));
    assert!(
        matches!(session.state.modal, Some(Modal::Limits(_))),
        "the editor stays so the numbers can be read"
    );
    assert_eq!(keep(&backend), Some(50));
}

#[test]
fn applying_without_changing_anything_says_so() {
    let (backend, system) = (machine(), system());
    let mut session = editing(&backend, &system, admin());
    session.press(Key::Enter);
    assert!(session.state.modal.is_none());
    assert!(format!("{:?}", session.state.notice).contains("Nothing was changed"));
}

#[test]
fn the_health_row_says_what_enter_does() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('2'));
    let at = session
        .state
        .health
        .rows()
        .iter()
        .position(|r| matches!(r.action, RowAction::EditLimits(_)))
        .unwrap();
    session.state.health.cursor = at;
    assert!(flat(&session)
        .contains("Enter: change what it keeps U: stop protecting it (deletes its snapshots)"));
}
