// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Putting every area back from the restore prompt: the offer, what `A` does and does not touch, the
//! way back, and that a snapshot taken alone is not offered anything. Drawn at 80x24.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::InMemoryBackend;
use snapall_core::{plan_snapshots, run_plan, ConfigName, Description, Point, SnapshotBackend};
use snapall_tui::model::{Button, Click, Key, Modal, Msg};
use snapall_tui::{view, Session};

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};
const JARGON: [&str; 5] = ["subvolume", "btrfs", "snapper", "filesystem", "file system"];

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

/// The screen as running text: borders gone and line breaks gone, so a phrase is found wherever it
/// happens to wrap.
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

/// docs, home and the system (root), one action snapshots all three as one restore point.
fn three_areas() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["docs", "home", "root"]).with_root_config("root");
    backend.write_file("docs", "/mnt/docs/a.txt", "docs, then");
    backend.write_file("home", "/mnt/home/b.txt", "home, then");
    let plan = plan_snapshots(&backend, None).unwrap();
    let point = Point::parse("20261025T100000Z").unwrap();
    run_plan(
        &backend,
        &plan,
        &Description::parse("before theming").unwrap(),
        Some(&point),
    );
    backend.write_file("docs", "/mnt/docs/a.txt", "docs, ruined");
    backend.write_file("home", "/mnt/home/b.txt", "home, ruined");
    backend
}

/// Open the restore prompt on the newest snapshot of `area` (areas are listed docs, home, root).
fn open_restore(session: &mut Session<'_>, down: usize) {
    for _ in 0..down {
        session.press(Key::Down);
    }
    session.press(Key::Right);
    session.press(Key::Char('u'));
}

#[test]
fn a_snapshot_taken_with_others_offers_to_put_them_all_back_and_says_what_it_is() {
    let backend = three_areas();
    let mut session = Session::new(&backend);
    open_restore(&mut session, 1); // home
    let text = flat(&screen(&session));
    for must in [
        "restore point 20261025T100000Z (before theming)",
        "taken in 3 areas: docs, home, root",
        "Press A to put all 2 that differ back at once",
        "The running system is left alone",
        "[ A: Restore all 2 ]",
    ] {
        assert!(
            text.contains(must),
            "`{must}` missing or cut off at 80x24:\n{text}"
        );
    }
    let lower = text.to_lowercase();
    for word in JARGON {
        assert!(!lower.contains(word), "`{word}` in:\n{text}");
    }
}

#[test]
fn a_puts_every_area_that_differs_back_leaves_the_system_alone_and_names_the_way_back() {
    let backend = three_areas();
    backend.write_file("root", "/etc/x", "system, changed after");
    let mut session = Session::new(&backend);
    open_restore(&mut session, 1);
    session.press(Key::Char('A'));
    session.confirm();

    assert!(session.state.modal.is_none(), "the prompt closed");
    assert_eq!(
        backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
        Some("docs, then")
    );
    assert_eq!(
        backend.read_file("home", "/mnt/home/b.txt").as_deref(),
        Some("home, then")
    );
    assert_eq!(
        backend.read_file("root", "/etc/x").as_deref(),
        Some("system, changed after"),
        "the running system is untouched"
    );
    let note = format!("{:?}", session.state.notice);
    assert!(
        note.contains("Put 2 area(s) back to 20261025T100000Z")
            && note.contains("Undo it: restore point"),
        "{note}"
    );
}

#[test]
fn the_way_back_is_the_same_action_on_the_safety_point_it_names() {
    let backend = three_areas();
    let mut session = Session::new(&backend);
    open_restore(&mut session, 1);
    session.press(Key::Char('A'));
    session.confirm();
    assert_eq!(
        backend.read_file("home", "/mnt/home/b.txt").as_deref(),
        Some("home, then")
    );

    // the newest snapshot of home is now the safety snapshot taken just before: restore its point
    session.press(Key::Esc);
    let mut again = Session::new(&backend);
    open_restore(&mut again, 1);
    let text = flat(&screen(&again));
    assert!(
        text.contains("before restoring 20261025T100000Z"),
        "the safety point is named for what it undoes:\n{text}"
    );
    again.press(Key::Char('A'));
    again.confirm();
    assert_eq!(
        backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
        Some("docs, ruined"),
        "back to just before"
    );
    assert_eq!(
        backend.read_file("home", "/mnt/home/b.txt").as_deref(),
        Some("home, ruined")
    );
}

#[test]
fn a_snapshot_taken_alone_is_offered_no_restore_point_and_a_does_nothing() {
    let backend = InMemoryBackend::with_configs(&["docs", "home"]);
    backend.write_file("home", "/mnt/home/b.txt", "then");
    backend
        .create_snapshot(
            &ConfigName::parse("home").unwrap(),
            &Description::parse("on its own").unwrap(),
        )
        .unwrap();
    backend.write_file("home", "/mnt/home/b.txt", "ruined");
    let mut session = Session::new(&backend);
    open_restore(&mut session, 1);
    let text = screen(&session);
    assert!(
        !text.contains("restore point") && !text.contains("Restore all"),
        "{text}"
    );
    session.press(Key::Char('A'));
    session.confirm();
    assert!(
        matches!(session.state.modal, Some(Modal::Restore { .. })),
        "A is not a key of this prompt"
    );
    assert_eq!(
        backend.read_file("home", "/mnt/home/b.txt").as_deref(),
        Some("ruined")
    );
}

#[test]
fn when_everything_already_matches_there_is_nothing_to_offer() {
    let backend = three_areas();
    backend.write_file("docs", "/mnt/docs/a.txt", "docs, then");
    backend.write_file("home", "/mnt/home/b.txt", "home, then");
    let mut session = Session::new(&backend);
    open_restore(&mut session, 1);
    assert!(
        !screen(&session).contains("Restore all"),
        "{}",
        screen(&session)
    );
}

#[test]
fn the_button_explains_itself_and_works_by_mouse() {
    let backend = three_areas();
    let mut session = Session::new(&backend);
    open_restore(&mut session, 1);
    let line = screen(&session)
        .lines()
        .position(|l| l.contains("[ A: Restore all 2 ]"))
        .unwrap();
    let col = screen(&session)
        .lines()
        .nth(line)
        .unwrap()
        .find("A: Restore")
        .unwrap();
    let click = view::hit_test(
        &session.state,
        SCREEN,
        u16::try_from(col).unwrap(),
        u16::try_from(line).unwrap(),
    );
    assert_eq!(
        click,
        Some(snapall_tui::model::Click::Button(Button::RestoreAll))
    );
    session.dispatch(Msg::Hover(Some(Click::Button(Button::RestoreAll))));
    let text = view::button_help(Button::RestoreAll, &session.state);
    assert!(
        text.chars().count() <= 70,
        "it must fit the footer line: {text:?}"
    );
    assert!(
        screen(&session).contains(text),
        "the whole sentence is on screen:\n{}",
        screen(&session)
    );
    session.dispatch(Msg::Click(click.unwrap()));
    session.confirm();
    assert_eq!(
        backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
        Some("docs, then")
    );
}
