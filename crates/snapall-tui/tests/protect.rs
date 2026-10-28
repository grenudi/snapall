// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Protecting a part of the disk from inside the window: what it shows, what it asks, what it does,
//! and what it says when it cannot. Drawn at a standard 80x24.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::ports::testing::InMemoryBackend;
use snapall_core::vocabulary::explain::protect_summary;
use snapall_core::SnapshotBackend;
use snapall_tui::model::{update, Button, Click, Key, Modal, Msg, State};
use snapall_tui::{view, Offer, Session, UiContext};

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};
const JARGON: [&str; 6] = [
    "subvolume",
    "btrfs",
    "snapper",
    "config",
    "filesystem",
    "file system",
];

fn offer(mount_point: &str, title: &str, recommended: bool) -> Offer {
    Offer {
        mount_point: mount_point.into(),
        title: title.into(),
        summary: protect_summary(mount_point),
        recommended,
    }
}

fn context(administrator: bool, offers: Vec<Offer>) -> UiContext {
    UiContext {
        administrator,
        protect_hint: Some(
            "Home has no snapshots yet. Press p to see what that means and protect it.".into(),
        ),
        offers,
        ..UiContext::default()
    }
}

fn machine() -> InMemoryBackend {
    InMemoryBackend::with_areas(&["root"]).with_root_area("root")
}

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

fn two_offers() -> Vec<Offer> {
    vec![
        offer("/home", "Home", true),
        offer("/srv", "Served data", false),
    ]
}

#[test]
fn a_footer_line_says_at_the_start_that_something_worth_protecting_is_not_and_what_to_press() {
    let backend = machine();
    let session = Session::with_context(&backend, context(true, two_offers()));
    assert!(
        screen(&session).contains("Home has no snapshots yet. Press p"),
        "{}",
        screen(&session)
    );
    // nothing recommended is left, only something up to the person: no nagging
    let only_optional = Session::with_context(
        &backend,
        context(true, vec![offer("/srv", "Served data", false)]),
    );
    assert!(
        !screen(&only_optional).contains("Press p"),
        "{}",
        screen(&only_optional)
    );
}

#[test]
fn p_shows_what_protecting_means_one_part_at_a_time_and_fits_a_small_window() {
    let backend = machine();
    let mut session = Session::with_context(&backend, context(true, two_offers()));
    session.press(Key::Char('p'));
    let first = screen(&session);
    for must in [
        "Protect  (1 of 2)",
        "Protect Home (/home)?",
        "your own files",
        "Recommended:",
        "could not be undone",
        "never on a schedule",
        "does not copy, move or change any of your files",
        "[ Enter: Protect it ]",
        "[ Tab: Next ]",
        "[ Esc: Cancel ]",
    ] {
        assert!(
            first.contains(must),
            "`{must}` missing or cut off:\n{first}"
        );
    }
    session.press(Key::Tab);
    let second = screen(&session);
    assert!(
        second.contains("Protect  (2 of 2)")
            && second.contains("Protect Served data (/srv)?")
            && second.contains("Up to you:"),
        "{second}"
    );
    session.press(Key::Tab);
    assert!(screen(&session).contains("(1 of 2)"), "it goes round");
    for text in [&first, &second] {
        let lower = text.to_lowercase().replace(".snapshots", "");
        for word in JARGON {
            assert!(!lower.contains(word), "`{word}` in:\n{text}");
        }
    }
}

#[test]
fn enter_protects_it_for_real_schedules_off_the_list_updates_and_the_hint_goes() {
    let backend = machine();
    let mut session = Session::with_context(&backend, context(true, two_offers()));
    session.press(Key::Char('p'));
    session.press(Key::Enter);
    session.confirm();
    assert!(session.state.modal.is_none());
    let home = snapall_core::AreaName::parse("home").unwrap();
    assert!(
        backend.list_areas().unwrap().contains(&home),
        "the area was set up"
    );
    assert_eq!(
        backend.timeline_create("home"),
        Some(false),
        "snapshots on a schedule are off"
    );
    let note = format!("{:?}", session.state.notice);
    assert!(
        note.contains("Home (/home) is protected now") && note.contains("takes no snapshot yet")
            || note.contains("No snapshot exists yet"),
        "{note}"
    );
    let text = screen(&session);
    assert!(
        text.contains("home"),
        "the new area is in the list on the left:\n{text}"
    );
    assert_eq!(session.state.offers.len(), 1, "it left the offers");
    assert!(
        !text.contains("Home has no snapshots yet"),
        "the hint goes with the thing it was about"
    );
}

#[test]
fn a_failure_says_so_in_plain_words_leaves_nothing_changed_and_keeps_the_offer() {
    let backend = machine();
    backend.refuse_option_changes();
    let mut session = Session::with_context(&backend, context(true, two_offers()));
    session.press(Key::Char('p'));
    session.press(Key::Enter);
    session.confirm();
    let note = format!("{:?}", session.state.notice);
    assert!(
        note.contains("Could not protect Home") && note.contains("Nothing was changed"),
        "{note}"
    );
    assert_eq!(
        backend.list_areas().unwrap().len(),
        1,
        "what was started was undone"
    );
    assert_eq!(
        session.state.offers.len(),
        2,
        "still offered, so it can be tried again"
    );
}

#[test]
fn without_administrator_rights_it_explains_instead_of_failing_and_offers_no_protect_button() {
    let backend = machine();
    let mut session = Session::with_context(&backend, context(false, two_offers()));
    session.press(Key::Char('p'));
    let text = screen(&session);
    assert!(
        text.contains("administrator rights") && text.contains("sudo snapall tui"),
        "{text}"
    );
    assert!(!text.contains("[ Enter: Protect it ]"), "{text}");
    session.press(Key::Enter); // closes: there is nothing to confirm
    assert!(session.state.modal.is_none());
    assert_eq!(backend.list_areas().unwrap().len(), 1);
}

#[test]
fn with_nothing_to_offer_it_says_everything_is_protected_and_escape_leaves() {
    let backend = machine();
    let mut session = Session::with_context(&backend, context(true, Vec::new()));
    session.press(Key::Char('p'));
    let text = screen(&session);
    assert!(
        text.contains("has snapshots already") && text.contains("not a backup")
            || text.contains("same disk"),
        "{text}"
    );
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
}

#[test]
fn escape_changes_nothing_and_the_button_works_by_mouse_with_its_explanation_on_hover() {
    let backend = machine();
    let mut session = Session::with_context(&backend, context(true, two_offers()));
    session.press(Key::Char('p'));
    session.press(Key::Esc);
    assert_eq!(backend.list_areas().unwrap().len(), 1);

    let col_row = {
        let text = screen(&session);
        let (y, line) = text
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("p protect"))
            .expect("a protect button in the footer");
        (line.find("p protect").unwrap() as u16, y as u16)
    };
    let click = view::hit_test(&session.state, SCREEN, col_row.0 + 2, col_row.1);
    assert_eq!(click, Some(Click::Button(Button::Protect)));
    session.dispatch(Msg::Hover(Some(Click::Button(Button::Protect))));
    assert!(screen(&session)
        .contains("See which parts of your disk have no snapshots yet, and protect them"));
    session.dispatch(Msg::Click(click.unwrap()));
    assert!(matches!(
        session.state.modal,
        Some(Modal::Protect { index: 0 })
    ));
}

#[test]
fn like_every_action_that_starts_work_it_is_refused_while_snapall_is_busy() {
    let mut state = State::with_context(context(true, two_offers()));
    update(&mut state, Msg::Start);
    update(&mut state, Msg::HealthLoaded(Err(String::new())));
    assert_eq!(state.pending, 1);
    update(&mut state, Msg::Key(Key::Char('p')));
    assert!(state.modal.is_none(), "busy: no prompt opens");
}
