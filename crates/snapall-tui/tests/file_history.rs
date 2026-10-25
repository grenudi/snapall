// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The UI opened on one file, as a file manager's right-click opens it: how the file stood in each
//! snapshot, and putting a version back.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::InMemoryBackend;
use snapall_core::{ConfigName, Description, SnapshotBackend};
use snapall_tui::model::{Button, Click, Key, Modal, Msg, Pane, RestoreStage};
use snapall_tui::{view, FileTarget, Session, UiContext};

const FILE: &str = "/mnt/home/notes.txt";
const SIZE: Rect = Rect {
    x: 0,
    y: 0,
    width: 110,
    height: 24,
};

fn home() -> ConfigName {
    ConfigName::parse("home").unwrap()
}

fn snap(backend: &InMemoryBackend, config: &ConfigName, text: &str) {
    backend
        .create_snapshot(config, &Description::parse(text).unwrap())
        .unwrap();
}

/// notes.txt over time: not there (#1), "v1" (#2), "v2" (#3), and "v3" now, as in #4.
fn history() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["home"]);
    snap(&backend, &home(), "before the file existed");
    backend.write_file("home", FILE, "v1");
    snap(&backend, &home(), "first draft");
    backend.write_file("home", FILE, "v2");
    snap(&backend, &home(), "second draft");
    backend.write_file("home", FILE, "v3");
    snap(&backend, &home(), "as it is now");
    backend
}

fn open<'a>(backend: &'a InMemoryBackend, path: &str, config: &str) -> Session<'a> {
    let context = UiContext {
        boot_tip: None,
        restore_route: String::new(),
        file: Some(FileTarget {
            path: path.to_owned(),
            config: ConfigName::parse(config).unwrap(),
        }),
        ..UiContext::default()
    };
    Session::with_context(backend, context)
}

fn rows(session: &Session<'_>, width: u16, height: u16) -> Vec<String> {
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
        })
        .collect()
}

fn screen(session: &Session<'_>) -> String {
    rows(session, 110, 24).join("\n")
}

fn click_text(session: &mut Session<'_>, needle: &str, offset: u16) {
    let drawn = rows(session, 110, 24);
    let (y, line) = drawn
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .unwrap_or_else(|| panic!("`{needle}` is not on screen:\n{}", drawn.join("\n")));
    let col = line.find(needle).unwrap() as u16 + offset;
    if let Some(click) = view::hit_test(&session.state, SIZE, col, y as u16) {
        session.dispatch(Msg::Click(click));
    }
}

#[test]
fn it_opens_on_the_file_and_says_how_each_snapshot_compares_with_it() {
    let backend = history();
    let session = open(&backend, FILE, "home");
    let text = screen(&session);
    assert!(text.contains("history of /mnt/home/notes.txt"), "{text}");
    assert!(text.contains("notes.txt in home"), "{text}");
    assert!(
        !text.contains("Protected areas"),
        "there is no configs pane when opened on a file:\n{text}"
    );
    // newest first, each with the words for how it compares
    let order: Vec<usize> = [
        "as it is now",
        "second draft",
        "first draft",
        "before the file existed",
    ]
    .iter()
    .map(|d| text.find(d).unwrap_or_else(|| panic!("{d}\n{text}")))
    .collect();
    assert!(
        order.windows(2).all(|w| w[0] < w[1]),
        "newest first\n{text}"
    );
    for word in ["same as now", "differs from now", "did not exist"] {
        assert!(text.contains(word), "`{word}`\n{text}");
    }
}

#[test]
fn a_version_identical_to_the_file_or_from_before_it_existed_has_nothing_to_restore() {
    let backend = history();
    let mut session = open(&backend, FILE, "home");
    session.press(Key::Char('u')); // the newest: the same as now
    assert!(session.state.modal.is_none());
    assert!(
        format!("{:?}", session.state.notice).contains("already the same"),
        "{:?}",
        session.state.notice
    );
    session.press(Key::End); // the oldest: before the file existed
    session.press(Key::Enter);
    assert!(session.state.modal.is_none());
    assert!(
        format!("{:?}", session.state.notice).contains("did not exist"),
        "{:?}",
        session.state.notice
    );
    assert_eq!(backend.read_file("home", FILE).as_deref(), Some("v3"));
}

#[test]
fn restoring_a_version_asks_first_then_puts_the_file_back_and_the_list_catches_up() {
    let backend = history();
    let mut session = open(&backend, FILE, "home");
    session.press(Key::Down); // #3, "second draft", which had v2
    session.press(Key::Char('u'));
    let Some(Modal::Restore {
        path,
        stage: RestoreStage::Ready(preview),
        ..
    }) = &session.state.modal
    else {
        panic!("a prompt that has looked: {:?}", session.state.modal);
    };
    assert_eq!(path.as_deref(), Some(FILE));
    assert_eq!(preview.changes.len(), 1, "only this file");
    let text = screen(&session);
    assert!(text.contains("Put /mnt/home/notes.txt back"), "{text}");
    assert!(text.contains("snapshotted first"), "{text}");
    assert_eq!(
        backend.read_file("home", FILE).as_deref(),
        Some("v3"),
        "nothing yet"
    );

    session.press(Key::Enter);
    assert_eq!(backend.read_file("home", FILE).as_deref(), Some("v2"));
    let note = format!("{:?}", session.state.notice);
    assert!(
        note.contains("restored /mnt/home/notes.txt from #3"),
        "{note}"
    );
    let after = screen(&session);
    assert!(
        after.contains("before restoring #3"),
        "the safety snapshot is listed:\n{after}"
    );
    // second draft now matches the file; the newest version, taken before, now differs
    let versions = session.state.displayed_versions();
    let by_id = |id: u64| {
        versions
            .iter()
            .find(|v| v.snapshot.id.get() == id)
            .unwrap()
            .state
    };
    assert_eq!(by_id(3), snapall_core::FileState::Same);
    assert_eq!(by_id(4), snapall_core::FileState::Changed);
}

#[test]
fn escape_leaves_the_file_alone_and_takes_no_safety_snapshot() {
    let backend = history();
    let mut session = open(&backend, FILE, "home");
    session.press(Key::Down);
    session.press(Key::Char('u'));
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
    assert_eq!(backend.read_file("home", FILE).as_deref(), Some("v3"));
    assert_eq!(backend.snapshots().len(), 4);
}

#[test]
fn keys_that_belong_to_the_config_view_do_nothing_on_a_file() {
    let backend = history();
    let mut session = open(&backend, FILE, "home");
    for key in [
        Key::Char('c'),
        Key::Char('a'),
        Key::Char('d'),
        Key::Tab,
        Key::Char(' '),
        Key::Char('h'),
        Key::Char('l'),
    ] {
        session.press(key);
        assert!(session.state.modal.is_none(), "{key:?} opened something");
    }
    assert_eq!(
        backend.snapshots().len(),
        4,
        "nothing was created or deleted"
    );
}

#[test]
fn the_mouse_selects_a_version_and_its_buttons_restore_it() {
    let backend = history();
    let mut session = open(&backend, FILE, "home");
    click_text(&mut session, "second draft", 2);
    assert_eq!(
        session.state.file.as_ref().unwrap().cursor,
        1,
        "the second row is selected"
    );
    click_text(&mut session, "u restore this version", 0);
    assert!(
        matches!(session.state.modal, Some(Modal::Restore { .. })),
        "{:?}",
        session.state.modal
    );
    click_text(&mut session, "[ Enter: Restore ]", 4);
    assert_eq!(backend.read_file("home", FILE).as_deref(), Some("v2"));
}

#[test]
fn the_wheel_moves_the_cursor_and_clicks_behind_a_prompt_do_nothing() {
    let backend = history();
    let mut session = open(&backend, FILE, "home");
    session.dispatch(Msg::Scroll {
        pane: Pane::Snapshots,
        delta: 2,
    });
    assert_eq!(session.state.file.as_ref().unwrap().cursor, 2);
    session.dispatch(Msg::Scroll {
        pane: Pane::Snapshots,
        delta: -50,
    });
    assert_eq!(
        session.state.file.as_ref().unwrap().cursor,
        0,
        "clamped at the top"
    );
    session.press(Key::Down);
    session.press(Key::Char('u'));
    session.dispatch(Msg::Click(Click::Row {
        pane: Pane::Snapshots,
        index: 3,
    }));
    assert_eq!(
        session.state.file.as_ref().unwrap().cursor,
        1,
        "a click behind the prompt moved nothing"
    );
    session.dispatch(Msg::Click(Click::Button(Button::Quit)));
    assert!(!session.state.quit);
}

#[test]
fn a_click_lands_on_the_version_that_was_drawn_even_in_a_long_scrolled_list() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    backend.write_file("home", FILE, "x");
    for n in 0..60 {
        backend.write_file("home", FILE, &format!("content {n}"));
        snap(&backend, &home(), &format!("version {n}"));
    }
    let mut session = open(&backend, FILE, "home");
    session.press(Key::End);
    let drawn = rows(&session, 110, 24);
    let mut checked = 0;
    for (y, line) in drawn.iter().enumerate() {
        // a snapshot row says "version N"; the footer's "restore this version" is not one
        let Some(at) = line
            .match_indices("version ")
            .map(|(i, _)| i)
            .find(|&i| line[i + 8..].starts_with(|c: char| c.is_ascii_digit()))
        else {
            continue;
        };
        let described: String = line[at..]
            .chars()
            .take_while(|c| !matches!(c, '┃' | '│' | '┆'))
            .collect();
        let Some(Click::Row { index, .. }) = view::hit_test(&session.state, SIZE, 40, y as u16)
        else {
            panic!("row {y} is not clickable: {line}");
        };
        let hit = &session.state.displayed_versions()[index]
            .snapshot
            .description;
        assert_eq!(
            hit,
            described.trim(),
            "the click at row {y} must hit what is drawn there"
        );
        checked += 1;
    }
    assert!(checked >= 15, "only {checked} rows checked");
}

#[test]
fn a_file_of_the_system_root_can_be_restored_though_the_root_itself_cannot() {
    let backend = InMemoryBackend::with_configs(&["root"]).with_root_config("root");
    let root = ConfigName::parse("root").unwrap();
    backend.write_file("root", "/etc/fstab", "good");
    snap(&backend, &root, "base");
    backend.write_file("root", "/etc/fstab", "broken");
    let mut session = open(&backend, "/etc/fstab", "root");
    session.press(Key::Char('u'));
    let text = screen(&session);
    assert!(
        !text.contains("system root"),
        "a single file is not refused:\n{text}"
    );
    assert!(text.contains("[ Enter: Restore ]"), "{text}");
    session.press(Key::Enter);
    assert_eq!(
        backend.read_file("root", "/etc/fstab").as_deref(),
        Some("good")
    );
}

#[test]
fn a_file_no_config_covers_shows_the_reason_on_the_screen() {
    let backend = history();
    let session = open(&backend, "/etc/fstab", "home");
    let text = screen(&session);
    assert!(text.contains("no protected area contains"), "{text}");
}

#[test]
fn q_quits_and_a_file_with_no_snapshots_says_so() {
    let backend = InMemoryBackend::with_configs(&["home"]);
    let mut session = open(&backend, FILE, "home");
    assert!(screen(&session).contains("no snapshots of this area yet"));
    session.press(Key::Char('q'));
    assert!(session.state.quit);
}

#[test]
fn a_file_on_a_separate_subvolume_says_so_instead_of_listing_snapshots_that_cannot_hold_it() {
    let backend = InMemoryBackend::with_configs(&["root"])
        .with_root_config("root")
        .with_separate_subvolume("/mnt/home");
    let root = ConfigName::parse("root").unwrap();
    backend.write_file("root", "/mnt/home/u/notes.txt", "mine");
    snap(&backend, &root, "base");
    let session = open(&backend, "/mnt/home/u/notes.txt", "root");
    let text = screen(&session);
    assert!(text.contains("different part of the disk"), "{text}");
    assert!(
        !text.contains("did not exist"),
        "it must not claim the file was absent:\n{text}"
    );
}
