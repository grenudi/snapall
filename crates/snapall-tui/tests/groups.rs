// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Groups of areas in the window: see them, make one, change one, take one away, and snapshot one.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{
    plan_snapshots, run_plan, AreaName, Description, GroupName, Groups, SnapshotBackend,
    SystemProbe,
};
use snapall_tui::model::{GroupsScreen, Key, Modal};
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

fn area(name: &str) -> AreaName {
    AreaName::parse(name).unwrap()
}

/// Three protected areas, each with a snapshot.
fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_areas(&["data", "home", "root"]).with_root_area("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    run_plan(&backend, &plan, &Description::parse("first").unwrap(), None);
    backend
}

fn theming() -> Groups {
    let mut groups = Groups::default();
    groups
        .set(
            GroupName::parse("theming").unwrap(),
            vec![area("home"), area("data")],
        )
        .unwrap();
    groups
}

fn admin() -> UiContext {
    UiContext {
        administrator: true,
        ..UiContext::default()
    }
}

fn count(backend: &InMemoryBackend, name: &str) -> usize {
    backend.list_snapshots(&area(name)).unwrap().len()
}

fn type_text(session: &mut Session<'_>, text: &str) {
    for c in text.chars() {
        session.press(Key::Char(c));
    }
}

#[test]
fn g_opens_the_groups_which_start_with_all_and_say_what_a_group_is() {
    let (backend, system) = (machine(), FakeSystem::new().with_groups(theming()));
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('g'));
    assert!(matches!(
        session.state.modal,
        Some(Modal::Groups(GroupsScreen::List { cursor: 0 }))
    ));
    let text = flat(&session);
    for must in [
        "Groups of areas",
        "A group is a name for some of your areas",
        "all every protected area (data, home, root)",
        "theming home, data",
        "n: a new group",
    ] {
        assert!(text.contains(must), "`{must}` missing:\n{text}");
    }
    let screen = rows(&session);
    assert!(screen[23].contains("q quit"), "the key line is left alone");
}

#[test]
fn a_new_group_is_named_its_areas_chosen_and_it_is_saved() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('g'));
    session.press(Key::Char('n'));
    type_text(&mut session, "games");
    session.press(Key::Enter);
    assert!(
        flat(&session).contains("The areas of games"),
        "{}",
        flat(&session)
    );
    session.press(Key::Down); // home
    session.press(Key::Char(' '));
    session.press(Key::Enter);
    session.confirm();
    let saved = system.groups().unwrap();
    assert_eq!(
        saved.get(&GroupName::parse("games").unwrap()).unwrap(),
        [area("home")]
    );
    assert!(
        flat(&session).contains("games home"),
        "back at the list, with the new group in it"
    );
    assert_eq!(
        session.state.pending, 0,
        "nothing is left in flight once it is saved"
    );
    assert!(
        session.state.doing.is_empty(),
        "and nothing is announced as being done: {:?}",
        session.state.doing
    );
    assert!(
        system
            .events()
            .iter()
            .any(|e| format!("{e:?}").contains("Saved the groups of areas")),
        "it is in the record"
    );
}

#[test]
fn a_bad_name_says_why_and_a_group_without_areas_is_not_saved() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('g'));
    session.press(Key::Char('n'));
    type_text(&mut session, "Bad Name");
    session.press(Key::Enter);
    assert!(
        flat(&session).contains("cannot be a group name"),
        "{}",
        flat(&session)
    );
    for _ in 0..8 {
        session.press(Key::Backspace);
    }
    type_text(&mut session, "all");
    session.press(Key::Enter);
    assert!(flat(&session).contains("always there and cannot be changed"));
    for _ in 0..3 {
        session.press(Key::Backspace);
    }
    type_text(&mut session, "empty");
    session.press(Key::Enter);
    session.press(Key::Enter); // nothing ticked
    assert!(flat(&session).contains("Choose at least one area"));
    assert!(system.groups().unwrap().is_empty(), "nothing was saved");
}

#[test]
fn changing_a_groups_areas_starts_from_what_it_has_and_taking_one_away_leaves_the_areas() {
    let (backend, system) = (machine(), FakeSystem::new().with_groups(theming()));
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('g'));
    session.press(Key::Down); // theming
    session.press(Key::Enter);
    let text = flat(&session);
    assert!(
        text.contains("[x] data") && text.contains("[x] home") && text.contains("[ ] root"),
        "{text}"
    );
    session.press(Key::Char(' ')); // data out
    session.press(Key::Enter);
    session.confirm();
    assert_eq!(
        system
            .groups()
            .unwrap()
            .get(&GroupName::parse("theming").unwrap())
            .unwrap(),
        [area("home")]
    );

    session.press(Key::Char('d'));
    session.confirm();
    assert!(system.groups().unwrap().is_empty(), "the group is gone");
    assert_eq!(
        count(&backend, "home"),
        1,
        "the area and its snapshots are untouched"
    );
}

#[test]
fn all_cannot_be_changed_or_taken_away_and_says_so() {
    let (backend, system) = (machine(), FakeSystem::new().with_groups(theming()));
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('g'));
    session.press(Key::Enter);
    assert!(format!("{:?}", session.state.notice).contains("cannot be changed"));
    session.press(Key::Char('d'));
    assert!(format!("{:?}", session.state.notice).contains("cannot be taken away"));
    assert_eq!(system.groups().unwrap(), theming());
}

#[test]
fn without_administrator_rights_saving_says_so_and_changes_nothing() {
    let (backend, system) = (machine(), FakeSystem::new().with_groups(theming()));
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('g'));
    session.press(Key::Down);
    session.press(Key::Char('d'));
    assert!(format!("{:?}", session.state.notice).contains("needs administrator rights"));
    assert_eq!(system.groups().unwrap(), theming());
}

#[test]
fn tab_in_the_snapshot_prompt_chooses_all_or_a_group_and_says_what_is_included() {
    let (backend, system) = (
        machine(),
        FakeSystem::new()
            .with_groups(theming())
            .at("2026-10-25 12:00:00"),
    );
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('a'));
    assert!(flat(&session).contains("New snapshot of ALL protected areas (data, home, root)"));
    assert!(flat(&session).contains("Tab: all the areas, or one of your groups"));
    session.press(Key::Tab);
    assert!(
        flat(&session).contains("New snapshot of the group theming (home, data)"),
        "{}",
        flat(&session)
    );
    session.press(Key::Enter);
    assert_eq!(
        (
            count(&backend, "home"),
            count(&backend, "data"),
            count(&backend, "root")
        ),
        (2, 2, 1),
        "only the group"
    );
    // and round again to all
    session.press(Key::Char('a'));
    session.press(Key::Tab);
    session.press(Key::Tab);
    assert!(flat(&session).contains("ALL protected areas"));
}

#[test]
fn a_group_naming_an_area_that_is_no_longer_protected_is_refused_with_its_name() {
    let mut stale = theming();
    stale
        .set(
            GroupName::parse("stale").unwrap(),
            vec![area("home"), area("vanished")],
        )
        .unwrap();
    let (backend, system) = (machine(), FakeSystem::new().with_groups(stale));
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('a'));
    session.press(Key::Tab); // stale comes before theming
    session.press(Key::Enter);
    let text = flat(&session);
    assert!(
        text.contains("names vanished, which is no longer protected"),
        "{text}"
    );
    assert!(
        session.state.modal.is_some(),
        "the prompt stays so it can be changed"
    );
    assert_eq!(count(&backend, "home"), 1, "nothing was snapshotted");
}

#[test]
fn a_groups_file_that_cannot_be_read_is_shown_and_never_overwritten() {
    let backend = machine();
    let system = FakeSystem::new();
    let mut session = Session::with_system(&backend, &system, admin());
    session.state.groups_error =
        Some("/etc/snapall/groups: line 2: expected `name: area, area`".into());
    session.press(Key::Char('g'));
    assert!(flat(&session).contains("The groups could not be read: /etc/snapall/groups: line 2"));
    session.press(Key::Char('n'));
    assert!(
        matches!(
            session.state.modal,
            Some(Modal::Groups(GroupsScreen::List { .. }))
        ),
        "it does not go on"
    );
    assert!(format!("{:?}", session.state.notice).contains("left alone"));
}

#[test]
fn tabs_still_count_from_the_top_with_home_now_that_g_is_the_groups() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = Session::with_system(&backend, &system, admin());
    session.press(Key::Char('2'));
    session.press(Key::End);
    session.press(Key::Home);
    assert_eq!(session.state.health.cursor, 0);
    session.press(Key::Char('g'));
    assert!(
        matches!(session.state.modal, Some(Modal::Groups(_))),
        "g opens the groups from any tab"
    );
}
