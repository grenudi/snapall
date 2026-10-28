// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What ends up on screen, rendered into ratatui's in-memory test terminal.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use snapall_core::testing::InMemoryBackend;
use snapall_core::{AreaName, Description, SnapshotBackend};
use snapall_tui::model::Key;
use snapall_tui::{view, Session};

fn screen(session: &Session<'_>, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| view::render(&session.state, frame))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn backend_with_data() -> InMemoryBackend {
    let backend = InMemoryBackend::with_areas(&["root", "home"]);
    for (area, text) in [
        ("home", "before theming"),
        ("home", "after theming"),
        ("root", "kernel"),
    ] {
        backend
            .create_snapshot(
                &AreaName::parse(area).unwrap(),
                &Description::parse(text).unwrap(),
            )
            .unwrap();
    }
    backend
}

#[test]
fn the_main_screen_shows_areas_snapshots_and_key_hints() {
    let backend = backend_with_data();
    let session = Session::new(&backend);
    let text = screen(&session, 100, 20);

    assert!(text.contains("snapall"), "{text}");
    assert!(text.contains("Protected areas"), "{text}");
    assert!(
        text.contains("home  (2)"),
        "loaded counts show in the list\n{text}"
    );
    assert!(
        text.contains("root  (-)"),
        "unvisited configs show a dash\n{text}"
    );
    assert!(text.contains("Snapshots of home (2)"), "{text}");
    assert!(
        text.contains("after theming") && text.contains("before theming"),
        "{text}"
    );
    let newest = text.find("after theming").unwrap();
    let oldest = text.find("before theming").unwrap();
    assert!(newest < oldest, "newest first\n{text}");
    assert!(
        text.contains("c create") && text.contains("q quit"),
        "{text}"
    );
}

#[test]
fn marked_snapshots_are_visible() {
    let backend = backend_with_data();
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    session.press(Key::Char(' '));
    let text = screen(&session, 100, 20);
    assert!(
        text.lines()
            .any(|l| l.contains('*') && l.contains("after theming")),
        "{text}"
    );
}

#[test]
fn the_create_prompt_shows_its_target_and_the_text_typed_so_far() {
    let backend = backend_with_data();
    let mut session = Session::new(&backend);
    session.press(Key::Char('c'));
    for c in "my note".chars() {
        session.press(Key::Char(c));
    }
    let text = screen(&session, 100, 20);
    assert!(text.contains("New snapshot of home"), "{text}");
    assert!(text.contains("my note"), "{text}");
    assert!(
        text.contains("[ Esc: Cancel ]") && text.contains("[ Enter: Create ]"),
        "{text}"
    );
}

#[test]
fn the_all_areas_prompt_names_every_area() {
    let backend = backend_with_data();
    let mut session = Session::new(&backend);
    session.press(Key::Char('a'));
    let text = screen(&session, 100, 20);
    assert!(text.contains("ALL protected areas (home, root)"), "{text}");
}

#[test]
fn the_delete_confirmation_names_what_will_go() {
    let backend = backend_with_data();
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    session.press(Key::Char('d'));
    let text = screen(&session, 100, 20);
    assert!(text.contains("Delete 1 snapshot(s) of home?"), "{text}");
    assert!(text.contains("#2"), "{text}");
    assert!(text.contains("cannot be undone"), "{text}");
}

#[test]
fn errors_and_an_empty_snapper_are_shown_in_words() {
    let backend = InMemoryBackend::with_areas(&[]);
    let session = Session::new(&backend);
    let text = screen(&session, 120, 12);
    assert!(text.contains("no protected areas yet"), "{text}");
    assert!(
        text.contains("no protected areas")
            && !text.contains("create-config")
            && !text.contains("snapper"),
        "plain words, no raw command: {text}"
    );
}

#[test]
fn it_never_panics_however_small_the_terminal_is() {
    let backend = backend_with_data();
    let mut session = Session::new(&backend);
    for modal_key in [None, Some('c'), Some('a'), Some('?')] {
        if let Some(c) = modal_key {
            session.press(Key::Char(c));
        }
        for width in [0, 1, 2, 5, 10, 30, 80] {
            for height in [0, 1, 2, 3, 5, 12] {
                let _ = screen(&session, width, height);
            }
        }
        session.press(Key::Esc);
    }
}

#[test]
#[ignore = "prints the screen for a human; run with --ignored --nocapture"]
fn preview() {
    let backend = backend_with_data();
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    session.press(Key::Char(' '));
    println!("{}", screen(&session, 90, 16));
    session.press(Key::Char('d'));
    println!("{}", screen(&session, 90, 16));
}

#[test]
fn the_header_says_so_while_work_is_in_flight_and_stays_quiet_otherwise() {
    let backend = backend_with_data();
    let mut session = Session::new(&backend);
    assert!(
        !screen(&session, 100, 12).contains("working"),
        "idle: no indicator"
    );

    session.state.pending = 1;
    let first = screen(&session, 100, 12);
    assert!(first.contains("working"), "{first}");

    // The indicator moves, so a frozen-looking screen is distinguishable from a working one.
    session.state.spinner += 1;
    let second = screen(&session, 100, 12);
    assert_ne!(
        first.lines().next(),
        second.lines().next(),
        "the spinner advanced"
    );

    session.state.quit = true;
    assert!(
        screen(&session, 100, 12).contains("finishing"),
        "quitting waits for the work"
    );
}
