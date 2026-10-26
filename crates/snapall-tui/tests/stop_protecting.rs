// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Stopping the protection of an area: the one thing snapall does that cannot be taken back, so it says
//! what it will do, wants the name typed out, and refuses the system area.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{
    plan_snapshots, run_plan, ConfigName, Description, GroupName, Groups, SnapshotBackend,
};
use snapall_tui::model::{Key, Modal};
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

fn area(name: &str) -> ConfigName {
    ConfigName::parse(name).unwrap()
}

/// Three areas, the last the system, each with two snapshots.
fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["data", "home", "root"]).with_root_config("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    for note in ["one", "two"] {
        run_plan(&backend, &plan, &Description::parse(note).unwrap(), None);
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

fn type_text(session: &mut Session<'_>, text: &str) {
    for c in text.chars() {
        session.press(Key::Char(c));
    }
}

/// `U` on the row of `config` in the Health tab.
fn asking_about<'a>(
    backend: &'a InMemoryBackend,
    system: &'a FakeSystem,
    config: &str,
    context: UiContext,
) -> Session<'a> {
    let mut session = Session::with_system(backend, system, context);
    session.press(Key::Char('2'));
    let at = session
        .state
        .health
        .rows()
        .iter()
        .position(|row| row.area.as_ref() == Some(&area(config)))
        .expect("the area has a row");
    session.state.health.cursor = at;
    session.press(Key::Char('U'));
    session
}

fn protected(backend: &InMemoryBackend) -> Vec<String> {
    backend
        .list_configs()
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn the_question_says_what_goes_what_stays_and_that_it_cannot_be_undone() {
    let (backend, system) = (machine(), system());
    let session = asking_about(&backend, &system, "home", admin());
    assert!(matches!(
        session.state.modal,
        Some(Modal::StopProtecting(_))
    ));
    let text = flat(&session);
    for must in [
        "Stop protecting home?",
        "will no longer be protected. Its 2 snapshots will be deleted for good. The files in it are not touched.",
        "2 restore points include it and will no longer put it back.",
        "This cannot be undone. Protecting it again later starts fresh, with no snapshots.",
        "To go ahead, type its name (home):",
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
fn nothing_happens_until_the_name_is_typed_exactly() {
    let (backend, system) = (machine(), system());
    let mut session = asking_about(&backend, &system, "home", admin());
    session.press(Key::Enter);
    assert!(flat(&session).contains("Type its name exactly: home"));
    type_text(&mut session, "hom");
    session.press(Key::Enter);
    assert!(
        matches!(session.state.modal, Some(Modal::StopProtecting(_))),
        "a part of the name is not the name"
    );
    type_text(&mut session, "eX");
    session.press(Key::Enter);
    assert!(
        matches!(session.state.modal, Some(Modal::StopProtecting(_))),
        "nor is the name and more"
    );
    assert_eq!(
        protected(&backend),
        ["data", "home", "root"],
        "nothing was removed"
    );
    // a `y`, the answer that is enough elsewhere, is not the name
    session.press(Key::Esc);
    let mut again = asking_about(&backend, &system, "home", admin());
    type_text(&mut again, "y");
    again.press(Key::Enter);
    assert_eq!(protected(&backend), ["data", "home", "root"]);
}

#[test]
fn typing_the_name_stops_the_protection_deletes_its_snapshots_and_leaves_the_rest() {
    let (backend, system) = (machine(), system());
    let mut session = asking_about(&backend, &system, "home", admin());
    type_text(&mut session, "home");
    session.press(Key::Enter);
    assert!(session.state.modal.is_none());
    assert_eq!(protected(&backend), ["data", "root"]);
    assert_eq!(
        backend.list_snapshots(&area("data")).unwrap().len(),
        2,
        "the other areas keep theirs"
    );
    assert!(format!("{:?}", session.state.notice)
        .contains("is no longer protected: 2 snapshots deleted, its files are untouched"));
    assert_eq!(session.state.pending, 0);
    // the lists and the Health tab no longer have it
    assert!(!session
        .state
        .health
        .rows()
        .iter()
        .any(|row| row.area.as_ref() == Some(&area("home"))));
    assert!(
        system
            .events()
            .iter()
            .any(|e| format!("{e:?}").contains("Stopped protecting home")),
        "it is in the record"
    );
}

#[test]
fn the_system_area_is_refused_with_the_reason_and_nothing_is_removed() {
    let (backend, system) = (machine(), system());
    let mut session = asking_about(&backend, &system, "root", admin());
    let text = flat(&session);
    assert!(
        text.contains("snapall will not stop protecting the system area"),
        "{text}"
    );
    assert!(
        text.contains("how you get back from a bad update"),
        "{text}"
    );
    assert!(
        !text.contains("To go ahead, type its name"),
        "there is nothing to type"
    );
    type_text(&mut session, "root"); // typing does nothing
    session.press(Key::Enter);
    assert!(session.state.modal.is_none(), "Enter just closes");
    assert_eq!(protected(&backend), ["data", "home", "root"]);
}

#[test]
fn without_administrator_rights_it_says_so_and_changes_nothing() {
    let (backend, system) = (machine(), system());
    let mut session = asking_about(&backend, &system, "home", UiContext::default());
    type_text(&mut session, "home");
    session.press(Key::Enter);
    assert!(format!("{:?}", session.state.notice).contains("needs administrator rights"));
    assert!(
        matches!(session.state.modal, Some(Modal::StopProtecting(_))),
        "it stays, so it can be read"
    );
    assert_eq!(protected(&backend), ["data", "home", "root"]);
}

#[test]
fn a_group_that_names_the_area_is_told_about_and_then_says_it_is_no_longer_protected() {
    let mut groups = Groups::default();
    groups
        .set(
            GroupName::parse("theming").unwrap(),
            vec![area("home"), area("data")],
        )
        .unwrap();
    let (backend, system) = (machine(), system().with_groups(groups));
    let mut session = asking_about(&backend, &system, "home", admin());
    assert!(flat(&session).contains(
        "The group theming names it, and will say it is no longer protected until you change it."
    ));
    type_text(&mut session, "home");
    session.press(Key::Enter);
    session.press(Key::Char('g'));
    assert!(
        flat(&session).contains("home (no longer protected)"),
        "{}",
        flat(&session)
    );
}

#[test]
fn escape_leaves_everything_as_it_was_and_u_elsewhere_says_what_it_is_for() {
    let (backend, system) = (machine(), system());
    let mut session = asking_about(&backend, &system, "home", admin());
    type_text(&mut session, "home");
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
    assert_eq!(protected(&backend), ["data", "home", "root"]);

    session.state.health.cursor = 0; // a row that is not an area
    session.press(Key::Char('U'));
    assert!(session.state.modal.is_none());
    assert!(format!("{:?}", session.state.notice).contains("choose one of the areas"));
}

#[test]
fn the_health_row_says_how_to_stop_protecting_it() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('2'));
    let at = session
        .state
        .health
        .rows()
        .iter()
        .position(|r| r.area.as_ref() == Some(&area("home")))
        .unwrap();
    session.state.health.cursor = at;
    assert!(flat(&session).contains("U: stop protecting it (deletes its snapshots)"));
}
