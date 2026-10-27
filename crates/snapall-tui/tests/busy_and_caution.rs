// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The window says what it is doing and for how long while work is in flight, and says what a restore
//! also puts back, where the decision is made.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use snapall_core::testing::InMemoryBackend;
use snapall_core::{ConfigName, Description, SnapshotBackend};
use snapall_tui::model::{update, Effect, Key, Msg, State};
use snapall_tui::{view, Session, UiContext};

fn rows(state: &State) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|frame| view::render(state, frame)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..24)
        .map(|y| (0..80).map(|x| buffer[(x, y)].symbol()).collect::<String>())
        .collect()
}

fn flat(rows: &[String]) -> String {
    rows.join(" ")
        .chars()
        .map(|c| {
            if "│┌┐└┘╭╮╰╯─".contains(c) {
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

fn backend() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["home", "root"]).with_root_config("root");
    backend.write_file("home", "/mnt/home/a.txt", "one");
    backend
        .create_snapshot(
            &ConfigName::parse("home").unwrap(),
            &Description::parse("before").unwrap(),
        )
        .unwrap();
    backend.write_file("home", "/mnt/home/a.txt", "two");
    backend
}

/// A window that has asked for a snapshot of everything and is waiting for the answer.
fn waiting_for_snapshots(backend: &InMemoryBackend) -> State {
    let mut state = Session::new(backend).state.clone();
    update(&mut state, Msg::Key(Key::Char('a')));
    let effects = update(&mut state, Msg::Key(Key::Enter));
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Create { .. })),
        "{effects:?}"
    );
    state
}

#[test]
fn while_work_is_in_flight_the_window_says_what_it_is_doing_and_for_how_long() {
    let backend = backend();
    let mut state = waiting_for_snapshots(&backend);
    assert_eq!(state.doing, ["Taking a snapshot of home, root"]);
    for _ in 0..30 {
        update(&mut state, Msg::Tick);
    }
    let screen = rows(&state);
    assert!(
        screen[0].contains("working 3 s"),
        "the header counts: {}",
        screen[0]
    );
    assert!(
        screen[21].contains("Taking a snapshot of home, root... 3 s"),
        "the line under the status bar says what: {}",
        screen[21]
    );
    assert!(
        !screen[21].contains("this can take a while"),
        "not before there is reason to"
    );
}

#[test]
fn after_a_few_seconds_it_says_it_can_take_a_while_and_that_waiting_is_safe() {
    let backend = backend();
    let mut state = waiting_for_snapshots(&backend);
    for _ in 0..60 {
        update(&mut state, Msg::Tick);
    }
    let line = rows(&state)[21].clone();
    assert!(
        line.contains(
            "This can take a while; waiting is safe. Taking a snapshot of home, root (6 s)"
        ),
        "{line}"
    );
}

#[test]
fn closing_while_work_is_in_flight_says_it_waits_for_it() {
    let backend = backend();
    let mut state = waiting_for_snapshots(&backend);
    state.quit = true;
    let line = rows(&state)[21].clone();
    assert!(
        line.contains("Closing as soon as this has finished: Taking a snapshot"),
        "{line}"
    );
}

#[test]
fn when_the_work_is_done_the_line_goes_and_the_clock_starts_over() {
    let backend = backend();
    let mut state = waiting_for_snapshots(&backend);
    for _ in 0..20 {
        update(&mut state, Msg::Tick);
    }
    assert!(state.busy_ticks >= 20);
    update(&mut state, Msg::Created(Vec::new()));
    assert_eq!(state.pending, 0);
    assert!(
        state.doing.is_empty() && state.busy_ticks == 0,
        "{:?} {}",
        state.doing,
        state.busy_ticks
    );
    assert!(!rows(&state)[21].contains("Taking a snapshot of"));
}

#[test]
fn the_restore_prompt_says_what_else_goes_back_and_still_shows_its_buttons() {
    let backend = backend();
    let mut session = Session::with_context(&backend, UiContext::default());
    session.press(Key::Right);
    session.press(Key::Char('G'));
    session.press(Key::Char('u'));
    let screen = rows(&session.state);
    let text = flat(&screen);
    for must in [
        "Everything in the area goes back, including what programs keep there",
        "settings, saved sessions, databases and logs",
        "Close the programs that use those files first",
        "restore just that",
        "The current state is snapshotted first, so this can be undone.",
        "Restore",
        "Cancel",
    ] {
        assert!(
            text.contains(must),
            "`{must}` missing at 80x24:\n{}",
            screen.join("\n")
        );
    }
    assert!(screen[23].contains("q quit"), "the key line is not covered");
}
