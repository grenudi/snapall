// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The tabs, the status bar under every view, the Health tab's explanations and its one action, and the
//! Activity tab's record. Drawn at 80x24.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{plan_snapshots, run_plan, Description, Topic};
use snapall_tui::model::{Click, Key, Msg};
use snapall_tui::{view, Session, Tab, UiContext};

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

fn screen(session: &Session<'_>) -> String {
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
        .join("\n")
}

fn flat(screen: &str) -> String {
    screen
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

fn row(screen: &str, index: usize) -> String {
    screen.lines().nth(index).unwrap_or_default().to_owned()
}

/// Home and the system, each with a snapshot.
fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["home", "root"]).with_root_config("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    run_plan(&backend, &plan, &Description::parse("first").unwrap(), None);
    backend
}

/// A disk at 91% and a cleanup job that is switched off: two things to look at.
fn troubled() -> FakeSystem {
    FakeSystem::new()
        .with_disk(100, 91)
        .at("2026-01-02 12:00:00")
        .with_service("snapper-cleanup.timer", "disabled", "inactive")
}

fn as_administrator() -> UiContext {
    UiContext {
        administrator: true,
        ..UiContext::default()
    }
}

#[test]
fn the_digits_switch_tabs_and_the_header_says_which_one_is_showing() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    let header = row(&screen(&session), 0);
    for name in ["1 Snapshots", "2 Health", "3 Activity"] {
        assert!(
            header.contains(name),
            "`{name}` missing from the tab bar: {header}"
        );
    }
    session.press(Key::Char('2'));
    assert_eq!(session.state.tab, Tab::Health);
    session.press(Key::Char('3'));
    assert_eq!(session.state.tab, Tab::Activity);
    session.press(Key::Char('1'));
    assert_eq!(session.state.tab, Tab::Snapshots);
}

#[test]
fn a_click_on_a_tab_name_goes_to_that_tab() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    let col = row(&screen(&session), 0).find("2 Health").unwrap();
    let click = view::hit_test(&session.state, SCREEN, u16::try_from(col + 2).unwrap(), 0);
    assert_eq!(click, Some(Click::Tab(Tab::Health)));
    session.dispatch(Msg::Click(click.unwrap()));
    assert_eq!(session.state.tab, Tab::Health);
}

#[test]
fn the_health_tab_leads_with_the_verdict_and_what_needs_a_look_and_the_numbers_are_a_key_away() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('2'));
    // the first screen is what matters most
    let top = flat(&screen(&session));
    for must in [
        "Health: 2 things need attention",
        "Needs a look",
        "is 91% full",
        "Old snapshots are not cleaned up automatically",
        "What this means",
    ] {
        assert!(
            top.contains(must),
            "`{must}` missing from the first screen at 80x24:\n{}",
            screen(&session)
        );
    }
    // the numbers follow, and the list scrolls to them with the cursor
    session.press(Key::End);
    let bottom = flat(&screen(&session));
    for must in [
        "Automatic cleanup",
        "Boot menu",
        "Boot menu: not used on this computer",
    ] {
        assert!(
            bottom.contains(must),
            "`{must}` missing after End:\n{}",
            screen(&session)
        );
    }
    assert!(
        bottom.contains("Your snapshots are not listed in a boot menu"),
        "the chosen row is explained"
    );
}

#[test]
fn the_status_bar_is_under_every_view_and_says_the_same_thing_in_each() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    for key in ['1', '2', '3'] {
        session.press(Key::Char(key));
        let bar = row(&screen(&session), 21);
        for item in [
            "2 snapshots",
            "disk 91% full",
            "cleanup off",
            "2 to look at",
        ] {
            assert!(
                bar.contains(item),
                "tab {key}: `{item}` missing from the status bar: {bar}"
            );
        }
    }
}

#[test]
fn a_click_on_a_status_item_goes_to_the_row_that_explains_it() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    let col = row(&screen(&session), 21).find("cleanup off").unwrap();
    let click = view::hit_test(&session.state, SCREEN, u16::try_from(col + 3).unwrap(), 21);
    assert_eq!(click, Some(Click::Chip(Topic::Cleanup)));
    session.dispatch(Msg::Click(click.unwrap()));
    assert_eq!(session.state.tab, Tab::Health);
    let chosen = &session.state.health.rows()[session.state.health.cursor];
    assert_eq!(chosen.topic, Topic::Cleanup);
    assert!(
        flat(&screen(&session)).contains("would pile up"),
        "its explanation is shown"
    );
}

#[test]
fn enter_on_the_cleanup_row_switches_it_on_and_the_whole_window_follows() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, as_administrator());
    let col = row(&screen(&session), 21).find("cleanup off").unwrap();
    session.dispatch(Msg::Click(Click::Chip(Topic::Cleanup)));
    let _ = col;
    session.press(Key::Enter);

    assert_eq!(system.jobs_switched_on(), ["snapper-cleanup.timer"]);
    let bar = row(&screen(&session), 21);
    assert!(
        bar.contains("cleanup on") && !bar.contains("cleanup off"),
        "the bar turned: {bar}"
    );
    assert!(
        bar.contains("1 to look at"),
        "only the disk is left to look at: {bar}"
    );
    session.press(Key::Char('3'));
    let text = flat(&screen(&session));
    assert!(
        text.contains("done Switched on automatic cleanup"),
        "{text}"
    );
}

#[test]
fn without_administrator_rights_it_says_so_in_plain_words_and_changes_nothing() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.dispatch(Msg::Click(Click::Chip(Topic::Cleanup)));
    session.press(Key::Enter);
    assert!(system.jobs_switched_on().is_empty());
    assert!(format!("{:?}", session.state.notice).contains("needs administrator rights"));
    // and a machine that refuses says why
    let refusing = troubled().refusing_jobs();
    let mut session = Session::with_system(&backend, &refusing, as_administrator());
    session.dispatch(Msg::Click(Click::Chip(Topic::Cleanup)));
    session.press(Key::Enter);
    assert!(format!("{:?}", session.state.notice)
        .contains("Could not switch automatic cleanup on: Access denied"));
    assert!(
        row(&screen(&session), 21).contains("cleanup off"),
        "nothing changed"
    );
}

#[test]
fn the_activity_tab_records_what_was_done_and_keeps_a_failure_visible_with_its_reason() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('3'));
    assert!(flat(&screen(&session)).contains("Nothing yet"));
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    session.press(Key::Char('3'));
    let text = flat(&screen(&session));
    assert!(
        text.contains("12:00:00 done Took a snapshot of home, root"),
        "{text}"
    );
    assert!(text.contains("2 snapshots taken"), "{text}");

    backend.refuse_snapshots_of("home");
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    session.press(Key::Char('3'));
    let text = flat(&screen(&session));
    assert!(
        text.contains("FAILED Took a snapshot of home, root"),
        "the newest is first, and it failed:\n{text}"
    );
    assert!(
        text.contains("It failed: home:"),
        "the reason stays on screen after the message is gone:\n{text}"
    );
}

#[test]
fn the_keys_that_act_on_the_snapshot_lists_do_nothing_while_another_tab_is_showing() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('2'));
    for key in [
        Key::Char('d'),
        Key::Char('u'),
        Key::Char('c'),
        Key::Char(' '),
        Key::Right,
    ] {
        session.press(key);
        assert!(
            session.state.modal.is_none(),
            "{key:?} opened something on a tab that does not show snapshots"
        );
    }
    session.press(Key::Char('?'));
    assert!(
        session.state.modal.is_some(),
        "the keys that belong to the whole window still work"
    );
}

#[test]
fn no_prompt_covers_the_status_bar_or_the_two_lines_beneath_it() {
    let backend = machine();
    let system = troubled();
    for (name, keys) in [
        ("describe", vec![Key::Char('c')]),
        ("help 1", vec![Key::Char('?')]),
        ("help 2", vec![Key::Char('?'), Key::Enter]),
        ("restore points", vec![Key::Char('R')]),
        ("delete", vec![Key::Right, Key::Char('d')]),
    ] {
        let mut session = Session::with_system(&backend, &system, UiContext::default());
        for key in keys {
            session.press(key);
        }
        assert!(session.state.modal.is_some(), "{name}");
        let text = screen(&session);
        assert!(
            row(&text, 21).contains("snapshots"),
            "{name}: the status bar is covered:\n{text}"
        );
        assert!(
            row(&text, 23).contains("q quit"),
            "{name}: the key line is covered"
        );
    }
}

#[test]
fn nothing_on_the_new_screens_uses_a_word_a_newcomer_was_not_told() {
    let backend = machine();
    let system = troubled();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    for key in ['2', '3'] {
        session.press(Key::Char(key));
        let lower = flat(&screen(&session)).to_lowercase();
        for word in ["subvolume", "btrfs", "config", "filesystem", "qgroup"] {
            assert!(!lower.contains(word), "`{word}` on tab {key}:\n{lower}");
        }
    }
}
