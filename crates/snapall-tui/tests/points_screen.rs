// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The restore points screen: list them, see what each would change, put every area back to one. Drawn
//! at 80x24, where nine footer buttons and a tall prompt both have to fit.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::InMemoryBackend;
use snapall_core::{plan_snapshots, run_plan, Description, Point};
use snapall_tui::model::{Button, Click, Key, Modal, Msg};
use snapall_tui::{view, Session};

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

/// docs, home and the system; two actions a moment apart, the content changing in between.
fn two_points() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["docs", "home", "root"]).with_root_config("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    backend.write_file("docs", "/mnt/docs/a.txt", "docs, first");
    backend.write_file("home", "/mnt/home/b.txt", "home, first");
    run_plan(
        &backend,
        &plan,
        &Description::parse("first moment").unwrap(),
        Some(&Point::parse("20261025T100000Z").unwrap()),
    );
    backend.write_file("docs", "/mnt/docs/a.txt", "docs, second");
    backend.write_file("home", "/mnt/home/b.txt", "home, second");
    run_plan(
        &backend,
        &plan,
        &Description::parse("second moment").unwrap(),
        Some(&Point::parse("20261025T110000Z").unwrap()),
    );
    backend.write_file("docs", "/mnt/docs/a.txt", "docs, ruined");
    backend.write_file("home", "/mnt/home/b.txt", "home, ruined");
    backend
}

#[test]
fn nine_footer_buttons_fit_eighty_columns_so_none_is_lost_off_the_end() {
    let backend = two_points();
    let session = Session::new(&backend);
    let text = screen(&session);
    let footer = text.lines().nth(23).unwrap();
    for button in [
        "c create",
        "a all",
        "d delete",
        "u restore",
        "R points",
        "r reload",
        "p protect",
        "? help",
        "q quit",
    ] {
        assert!(
            footer.contains(button),
            "`{button}` missing from the key line:\n{footer}"
        );
    }
}

#[test]
fn r_lists_the_points_newest_first_and_says_what_the_chosen_one_would_change_in_each_area() {
    let backend = two_points();
    let mut session = Session::new(&backend);
    session.press(Key::Char('R'));
    let text = flat(&screen(&session));
    for must in [
        "Restore points (1 of 2)",
        "> 20261025T110000Z",
        "second moment",
        "20261025T100000Z",
        "Putting every area back would:",
        "docs change 1 path(s): 1 reverted, 0 removed, 0 brought back",
        "root left alone: the running system is not put back",
        "[ Enter: Restore all 2 ]",
    ] {
        assert!(
            text.contains(must),
            "`{must}` missing or cut off at 80x24:\n{}",
            screen(&session)
        );
    }
    let newest = text.find("20261025T110000Z").unwrap();
    let older = text.find("20261025T100000Z").unwrap();
    assert!(newest < older, "newest first");
}

#[test]
fn enter_puts_every_area_back_leaves_the_system_and_names_the_way_back() {
    let backend = two_points();
    backend.write_file("root", "/etc/x", "system, now");
    let mut session = Session::new(&backend);
    session.press(Key::Char('R'));
    session.press(Key::Enter);
    assert!(session.state.modal.is_none());
    assert_eq!(
        backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
        Some("docs, second")
    );
    assert_eq!(
        backend.read_file("home", "/mnt/home/b.txt").as_deref(),
        Some("home, second")
    );
    assert_eq!(
        backend.read_file("root", "/etc/x").as_deref(),
        Some("system, now"),
        "the running system is untouched"
    );
    let note = format!("{:?}", session.state.notice);
    assert!(
        note.contains("Put 2 area(s) back to 20261025T110000Z")
            && note.contains("Undo it: restore point"),
        "{note}"
    );
}

#[test]
fn choosing_an_older_point_looks_up_and_restores_that_one_not_the_newest() {
    let backend = two_points();
    let mut session = Session::new(&backend);
    session.press(Key::Char('R'));
    session.press(Key::Down);
    let text = flat(&screen(&session));
    assert!(
        text.contains("Restore points (2 of 2)") && text.contains("> 20261025T100000Z"),
        "{text}"
    );
    assert!(
        text.contains("docs change 1 path(s)"),
        "the preview is for the point chosen:\n{text}"
    );
    session.press(Key::Enter);
    assert_eq!(
        backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
        Some("docs, first")
    );
    assert_eq!(
        backend.read_file("home", "/mnt/home/b.txt").as_deref(),
        Some("home, first")
    );
    // and the cursor cannot run off either end
    let mut again = Session::new(&backend);
    again.press(Key::Char('R'));
    again.press(Key::Up);
    // (the restore above made its safety snapshots a third point: the cursor is on the first of them)
    assert!(
        flat(&screen(&again)).contains("Restore points (1 of 3)"),
        "{}",
        screen(&again)
    );
}

#[test]
fn with_no_restore_points_it_says_what_makes_one() {
    let backend = InMemoryBackend::with_configs(&["docs", "home"]);
    let mut session = Session::new(&backend);
    session.press(Key::Char('R'));
    let text = flat(&screen(&session));
    assert!(
        text.contains("There are no restore points yet.")
            && text.contains("press a, or run sudo snapall"),
        "{text}"
    );
    session.press(Key::Enter);
    assert!(
        matches!(session.state.modal, Some(Modal::Points { .. })),
        "nothing to do: Enter does not close it"
    );
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
}

#[test]
fn a_point_that_already_matches_offers_nothing_and_enter_changes_nothing() {
    let backend = two_points();
    backend.write_file("docs", "/mnt/docs/a.txt", "docs, second");
    backend.write_file("home", "/mnt/home/b.txt", "home, second");
    let mut session = Session::new(&backend);
    session.press(Key::Char('R'));
    let text = flat(&screen(&session));
    assert!(
        text.contains("nothing: it already matches") && !text.contains("Restore all"),
        "{text}"
    );
    let before = backend.snapshots().len();
    session.press(Key::Enter);
    assert_eq!(
        backend.snapshots().len(),
        before,
        "no safety snapshot was taken for nothing"
    );
    assert!(matches!(session.state.modal, Some(Modal::Points { .. })));
}

#[test]
fn the_button_opens_it_by_mouse_explains_itself_and_the_prompt_leaves_the_footer_alone() {
    let backend = two_points();
    let mut session = Session::new(&backend);
    let text = screen(&session);
    let col = text.lines().nth(23).unwrap().find("R points").unwrap();
    let click = view::hit_test(&session.state, SCREEN, u16::try_from(col).unwrap(), 23);
    assert_eq!(click, Some(Click::Button(Button::Points)));
    session.dispatch(Msg::Hover(Some(Button::Points)));
    let help = view::button_help(Button::Points, &session.state);
    assert!(
        help.chars().count() <= 70 && screen(&session).contains(help),
        "{help:?}"
    );
    session.dispatch(Msg::Click(click.unwrap()));
    assert!(matches!(session.state.modal, Some(Modal::Points { .. })));
    // inside the prompt, the footer still shows what the buttons do
    session.dispatch(Msg::Hover(Some(Button::Confirm)));
    let rows: Vec<String> = screen(&session).lines().map(str::to_owned).collect();
    assert!(
        rows[22].contains("Put every area of the chosen restore point back"),
        "{}",
        rows[22]
    );
    assert!(rows[23].contains("q quit"), "{}", rows[23]);
}

#[test]
fn it_uses_no_word_a_newcomer_was_not_told() {
    let backend = two_points();
    let mut session = Session::new(&backend);
    session.press(Key::Char('R'));
    let lower = flat(&screen(&session)).to_lowercase();
    for word in ["subvolume", "btrfs", "snapper", "config", "filesystem"] {
        assert!(!lower.contains(word), "`{word}` on the screen:\n{lower}");
    }
}
