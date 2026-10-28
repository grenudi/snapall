// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The UI explains itself: every button says what it does, the help teaches what snapshots are for
//! someone who knows nothing, and no screen a newcomer can reach uses a word snapall has not
//! explained. Checked on the screen as it is drawn, in a standard 80x24 terminal.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::ports::testing::InMemoryBackend;
use snapall_core::{AreaName, Description, SnapshotBackend};
use snapall_tui::model::{Button, Click, Key, Modal, Msg};
use snapall_tui::{view, FileTarget, Session, UiContext};

const JARGON: [&str; 6] = [
    "subvolume",
    "btrfs",
    "snapper",
    "config",
    "filesystem",
    "file system",
];
const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

fn backend() -> InMemoryBackend {
    let backend = InMemoryBackend::with_areas(&["home", "root"]);
    let home = AreaName::parse("home").unwrap();
    backend.write_file("home", "/mnt/home/a.txt", "one");
    backend
        .create_snapshot(&home, &Description::parse("first").unwrap())
        .unwrap();
    backend.write_file("home", "/mnt/home/a.txt", "two");
    backend
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

fn assert_plain(what: &str, text: &str) {
    let lower = text.to_lowercase();
    for word in JARGON {
        assert!(
            !lower.contains(word),
            "`{word}` on the {what} screen:\n{text}"
        );
    }
}

#[test]
fn the_mouse_over_a_button_says_what_it_does_and_leaving_it_stops() {
    let backend = backend();
    let mut session = Session::new(&backend);
    session.dispatch(Msg::Hover(Some(Click::Button(Button::Delete))));
    let text = screen(&session);
    assert!(
        text.contains("Delete the marked snapshots for good. Your files are not touched"),
        "{text}"
    );
    session.dispatch(Msg::Hover(None));
    assert!(
        !screen(&session).contains("for good"),
        "the explanation goes when the mouse does"
    );
}

#[test]
fn every_button_a_person_can_click_has_an_explanation_that_fits_the_footer_in_every_context() {
    let backend = backend();
    let footer = [
        Button::Create,
        Button::CreateAll,
        Button::Delete,
        Button::Restore,
        Button::Reload,
        Button::Help,
        Button::Quit,
        Button::Points,
    ];
    let session = Session::new(&backend);
    for button in footer {
        let text = view::button_help(button, &session.state);
        assert!(
            !text.is_empty() && text.chars().count() <= 78,
            "{button:?}: {text:?}"
        );
        assert_plain("button", text);
    }
    // inside each prompt, Enter and Esc mean something specific, and say it
    let prompts: [(&[Key], &str); 3] = [
        (&[Key::Char('c')], "Take the snapshot now"),
        (&[Key::Right, Key::Char('u')], "Put the files back"),
        (&[Key::Right, Key::Char('d')], "Delete them for good"),
    ];
    for (keys, expected) in prompts {
        let mut session = Session::new(&backend);
        for key in keys {
            session.press(*key);
        }
        assert!(session.state.modal.is_some(), "{keys:?} opens a prompt");
        let confirm = view::button_help(Button::Confirm, &session.state);
        assert!(confirm.contains(expected), "{keys:?}: {confirm:?}");
        let cancel = view::button_help(Button::Cancel, &session.state);
        assert!(cancel.contains("without changing anything"), "{cancel:?}");
        for text in [confirm, cancel] {
            assert!(text.chars().count() <= 78, "{text:?}");
            assert_plain("prompt button", text);
        }
    }
    // on a single file, restoring says it is about that file
    let file = UiContext {
        boot_tip: None,
        restore_route: String::new(),
        file: Some(FileTarget {
            path: "/mnt/home/a.txt".into(),
            area: AreaName::parse("home").unwrap(),
        }),
        ..UiContext::default()
    };
    let session = Session::with_context(&backend, file);
    assert!(view::button_help(Button::Restore, &session.state).contains("this file"));
}

#[test]
fn the_help_teaches_what_snapshots_are_before_it_lists_keys_and_fits_a_small_terminal() {
    let backend = backend();
    let mut session = Session::new(&backend);
    session.press(Key::Char('?'));
    let first = screen(&session);
    for must in [
        "What is this?",
        "saved moment",
        "does not copy your files",
        "Protected area",
        "Snapshot",
        "Restore",
        "can itself be undone",
    ] {
        assert!(
            first.contains(must),
            "`{must}` is missing or cut off at 80x24:\n{first}"
        );
    }
    assert_plain("help (page 1)", &first);
    session.press(Key::Enter);
    let second = screen(&session);
    for must in [
        "Keys and buttons",
        "Take a snapshot of this area, or of every one",
        "Delete marked snapshots for good (you type destroy)",
        "Restore a snapshot",
        "Answer a question: y for a change, destroy for a loss",
        "hover a button to read what it does",
        "Close snapall (changes nothing)",
    ] {
        assert!(
            second.contains(must),
            "`{must}` is missing or cut off at 80x24:\n{second}"
        );
    }
    assert_plain("help (page 2)", &second);
    assert!(matches!(session.state.modal, Some(Modal::Help { page: 1 })));
}

#[test]
fn the_prompts_say_what_they_will_do_in_words_that_assume_nothing() {
    let backend = backend();
    let mut session = Session::new(&backend);
    session.press(Key::Char('c'));
    let describe = screen(&session);
    assert!(
        describe.contains("A short note to help you recognise this snapshot later"),
        "{describe}"
    );
    assert_plain("snapshot description", &describe);
    session.press(Key::Esc);
    session.press(Key::Right);
    session.press(Key::Char('d'));
    let delete = screen(&session);
    for must in [
        "Your files are not touched",
        "the space they held is freed",
        "This cannot be undone",
    ] {
        assert!(delete.contains(must), "`{must}` missing:\n{delete}");
    }
    assert_plain("delete", &delete);
}

#[test]
fn the_main_screen_calls_the_parts_of_the_disk_protected_areas() {
    let backend = backend();
    let session = Session::new(&backend);
    let text = screen(&session);
    assert!(
        text.contains("Protected areas") && !text.contains("Configs"),
        "{text}"
    );
    assert_plain("main", &text);
}

/// The line under a prompt is where the mouse-over explanation appears, and the line below it holds
/// the keys. A prompt tall enough to cover either would hide exactly what a newcomer needs.
#[test]
fn no_prompt_covers_the_two_footer_lines_at_24_rows() {
    let backend = backend();
    let prompts: [(&str, &[Key]); 7] = [
        ("snapshot description", &[Key::Char('c')]),
        ("delete", &[Key::Right, Key::Char('d')]),
        ("restore", &[Key::Right, Key::Char('u')]),
        ("help, page 1", &[Key::Char('?')]),
        ("help, page 2", &[Key::Char('?'), Key::Enter]),
        ("protect", &[Key::Char('p')]),
        ("restore points", &[Key::Char('R')]),
    ];
    for (name, keys) in prompts {
        let offers = vec![snapall_tui::Offer {
            mount_point: "/home".into(),
            title: "Home".into(),
            summary: snapall_core::vocabulary::explain::protect_summary("/home"),
            recommended: true,
        }];
        let mut session = Session::with_context(
            &backend,
            UiContext {
                offers,
                administrator: true,
                ..UiContext::default()
            },
        );
        for key in keys {
            session.press(*key);
        }
        assert!(session.state.modal.is_some(), "{name}: a prompt is open");
        // the explanation row has a button's explanation on it
        session.dispatch(Msg::Hover(Some(Click::Button(Button::Cancel))));
        let text = screen(&session);
        let rows: Vec<&str> = text.lines().collect();
        let (explanation, keys_row) = (rows[21], rows[23]);
        assert!(
            keys_row.contains("q quit"),
            "{name}: the key line is covered:\n{text}"
        );
        assert!(
            explanation.contains("Close this without changing anything"),
            "{name}: the explanation line is covered:\n{text}"
        );
    }
}
