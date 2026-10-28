// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The mouse, and restoring from the TUI.
//!
//! A click is a message that goes through the same code as the key it stands for, so these tests
//! are about three things: that a click lands on what was drawn (checked against the real rendered
//! screen, on a list long enough to scroll), that it cannot do what the key could not, and that a
//! restore asks before it acts and refuses the system root.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::ports::testing::InMemoryBackend;
use snapall_core::{AreaName, Description, SnapshotBackend};
use snapall_tui::model::{update, Button, Click, Key, Modal, Msg, Pane, RestoreStage, State};
use snapall_tui::{view, Session, UiContext};

fn name(s: &str) -> AreaName {
    AreaName::parse(s).unwrap()
}

/// `home` (with `count` snapshots and a file that changes after the first), `iso`, `root`: in the
/// order the list shows them, so `home` is the one selected at the start.
fn backend(count: u64) -> InMemoryBackend {
    let backend = InMemoryBackend::with_areas(&["home", "iso", "root"]);
    backend.write_file("home", "/mnt/home/a.txt", "one");
    for n in 0..count {
        backend
            .create_snapshot(
                &name("home"),
                &Description::parse(&format!("snap {n}")).unwrap(),
            )
            .unwrap();
    }
    backend.write_file("home", "/mnt/home/a.txt", "two");
    backend.write_file("home", "/mnt/home/b.txt", "new");
    backend
}

/// The screen as one string per row.
fn rows(state: &State, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| view::render(state, frame)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

fn click(session: &mut Session<'_>, size: Rect, col: u16, row: u16) {
    if let Some(click) = view::hit_test(&session.state, size, col, row) {
        session.dispatch(Msg::Click(click));
    }
}

const SIZE: Rect = Rect {
    x: 0,
    y: 0,
    width: 100,
    height: 24,
};

// ---- a click lands on what was drawn ------------------------------------------------------

#[test]
fn clicking_a_snapshot_row_selects_exactly_the_row_that_was_drawn_even_when_the_list_scrolls() {
    let backend = backend(60);
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    session.press(Key::Char('G')); // the oldest: the cursor is far past the first screenful
    let drawn = rows(&session.state, 100, 24);
    let mut checked = 0;
    for (y, line) in drawn.iter().enumerate() {
        // data rows are the ones carrying a "snap N" description
        let Some(at) = line.find("snap ") else {
            continue;
        };
        let described: String = line[at..]
            .chars()
            .take_while(|c| !matches!(c, '┃' | '│' | '┆'))
            .collect();
        let described = described.trim().to_owned();
        let col = 40; // inside the row, outside the mark column
        let Some(Click::Row {
            pane: Pane::Snapshots,
            index,
        }) = view::hit_test(&session.state, SIZE, col, y as u16)
        else {
            panic!("row {y} ({described}) is not clickable");
        };
        let hit = session.state.displayed_snapshots()[index]
            .description
            .clone();
        assert_eq!(
            hit, described,
            "the click at row {y} must hit the snapshot drawn there"
        );
        checked += 1;
    }
    assert!(
        checked >= 15,
        "a long list shows many rows: only {checked} checked"
    );
}

#[test]
fn clicking_a_area_row_selects_the_area_drawn_there_and_loads_its_snapshots() {
    let backend = backend(3);
    let mut session = Session::new(&backend);
    let drawn = rows(&session.state, 100, 24);
    let y = drawn
        .iter()
        .position(|l| l.contains("iso"))
        .expect("iso is listed") as u16;
    click(&mut session, SIZE, 5, y);
    assert_eq!(session.state.selected_area().unwrap().name, name("iso"));
    assert_eq!(session.state.focus, Pane::Areas);
    // it loaded: the pane's title names the config
    let screen = rows(&session.state, 100, 24).join("\n");
    assert!(screen.contains("Snapshots of iso"), "{screen}");
}

#[test]
fn every_footer_button_is_drawn_where_it_is_clickable_and_does_what_its_key_does() {
    let backend = backend(3);
    let session = Session::new(&backend);
    let drawn = rows(&session.state, 100, 24);
    // the buttons are on the last two lines of a 24-line screen: doing things first, then finding your way
    let expected = [
        ("c create", Button::Create, 22),
        ("a all", Button::CreateAll, 22),
        ("d delete", Button::Delete, 22),
        ("u restore", Button::Restore, 22),
        ("g groups", Button::Groups, 22),
        ("R points", Button::Points, 23),
        ("r reload", Button::Reload, 23),
        ("p protect", Button::Protect, 23),
        ("? help", Button::Help, 23),
        ("q quit", Button::Quit, 23),
    ];
    for (label, button, row) in expected {
        let col = drawn[row]
            .find(label)
            .unwrap_or_else(|| panic!("`{label}` is not drawn: {}", drawn[row]))
            as u16;
        let (first, last) = (col, col + label.len() as u16 - 1);
        // the button is the label and a space of padding each side: all of it answers a click
        for at in [first - 1, first, last, last + 1] {
            assert_eq!(
                view::hit_test(&session.state, SIZE, at, row as u16),
                Some(Click::Button(button)),
                "{label} at column {at}"
            );
        }
        // and the gap between one button and the next does not
        assert_eq!(
            view::hit_test(&session.state, SIZE, last + 2, row as u16),
            None,
            "the gap after {label}"
        );
    }
}

#[test]
fn clicking_create_opens_the_prompt_and_its_buttons_work_by_click() {
    let backend = backend(3);
    let mut session = Session::new(&backend);
    session.dispatch(Msg::Click(Click::Button(Button::Create)));
    assert!(matches!(session.state.modal, Some(Modal::Describe { .. })));

    // the prompt's buttons are drawn, and the cell under "Cancel" closes it
    let drawn = rows(&session.state, 100, 24);
    let (y, line) = drawn
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("[ Esc: Cancel ]"))
        .expect("a Cancel button");
    let col = line.find("[ Esc: Cancel ]").unwrap() as u16 + 3;
    click(&mut session, SIZE, col, y as u16);
    assert!(session.state.modal.is_none(), "Cancel closed the prompt");
    assert_eq!(backend.snapshots().len(), 3, "nothing was created");

    // ...and "Create" creates
    session.dispatch(Msg::Click(Click::Button(Button::Create)));
    let drawn = rows(&session.state, 100, 24);
    let (y, line) = drawn
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("[ Enter: Create ]"))
        .unwrap();
    click(
        &mut session,
        SIZE,
        line.find("[ Enter: Create ]").unwrap() as u16 + 3,
        y as u16,
    );
    assert!(session.state.modal.is_none());
    assert_eq!(
        backend.snapshots().len(),
        4,
        "one snapshot of home was created"
    );
}

// ---- a click cannot do what the key could not ---------------------------------------------

#[test]
fn clicks_behind_a_prompt_do_nothing_and_are_never_typed_into_it() {
    let backend = backend(3);
    let mut session = Session::new(&backend);
    session.press(Key::Char('c'));
    // "q quit" is visible behind the prompt, but a click there must not quit or type a 'q'
    session.dispatch(Msg::Click(Click::Button(Button::Quit)));
    assert!(!session.state.quit);
    match &session.state.modal {
        Some(Modal::Describe { input, .. }) => {
            assert!(input.is_empty(), "nothing typed: {input:?}")
        }
        other => panic!("the prompt should still be open: {other:?}"),
    }
    // and a row click behind it changes nothing either
    let before = session.state.area_cursor;
    session.dispatch(Msg::Click(Click::Row {
        pane: Pane::Areas,
        index: 2,
    }));
    assert_eq!(session.state.area_cursor, before);
}

#[test]
fn confirm_and_cancel_do_nothing_when_no_prompt_is_open() {
    let backend = backend(3);
    let mut session = Session::new(&backend);
    session.dispatch(Msg::Click(Click::Button(Button::Confirm)));
    session.dispatch(Msg::Click(Click::Button(Button::Cancel)));
    assert!(session.state.modal.is_none() && !session.state.quit);
}

#[test]
fn a_button_that_starts_work_is_refused_while_snapper_is_busy_exactly_like_its_key() {
    let mut state = State::default();
    update(&mut state, Msg::Start);
    update(&mut state, Msg::HealthLoaded(Err(String::new())));
    assert_eq!(state.pending, 1);
    update(&mut state, Msg::Click(Click::Button(Button::Create)));
    assert!(state.modal.is_none(), "busy: no prompt opens");
    assert!(
        format!("{:?}", state.notice).contains("busy"),
        "{:?}",
        state.notice
    );
    update(&mut state, Msg::Click(Click::Button(Button::Restore)));
    assert!(state.modal.is_none());
}

#[test]
fn the_mark_column_toggles_a_mark_without_moving_the_cursor_unlike_the_space_key() {
    let backend = backend(5);
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    session.dispatch(Msg::Click(Click::Mark { index: 2 }));
    assert_eq!(
        session.state.snapshot_cursor, 2,
        "the cursor stays on the clicked row"
    );
    assert_eq!(session.state.marked.len(), 1);
    session.dispatch(Msg::Click(Click::Mark { index: 2 }));
    assert!(session.state.marked.is_empty(), "a second click unmarks");
    // the first cells of a snapshot row really are the mark column in the drawn screen
    let drawn = rows(&session.state, 100, 24);
    let y = drawn.iter().position(|l| l.contains("snap 4")).unwrap() as u16;
    let snapshots = view::areas(SIZE).snapshots;
    assert!(matches!(
        view::hit_test(&session.state, SIZE, snapshots.x + 1, y),
        Some(Click::Mark { .. })
    ));
}

#[test]
fn the_wheel_moves_the_pane_under_the_pointer_and_leaves_keyboard_focus_alone() {
    let backend = backend(10);
    let mut session = Session::new(&backend);
    assert_eq!(session.state.focus, Pane::Areas);
    session.dispatch(Msg::Scroll {
        pane: Pane::Snapshots,
        delta: 3,
    });
    assert_eq!(session.state.snapshot_cursor, 3);
    assert_eq!(
        session.state.focus,
        Pane::Areas,
        "the keyboard focus did not move"
    );
    session.dispatch(Msg::Scroll {
        pane: Pane::Snapshots,
        delta: -100,
    });
    assert_eq!(session.state.snapshot_cursor, 0, "clamped at the top");
    session.dispatch(Msg::Scroll {
        pane: Pane::Areas,
        delta: 1,
    });
    assert_eq!(session.state.selected_area().unwrap().name, name("iso"));
    assert_eq!(view::pane_at(&session.state, SIZE, 5, 5), Some(Pane::Areas));
    assert_eq!(
        view::pane_at(&session.state, SIZE, 60, 5),
        Some(Pane::Snapshots)
    );
    assert_eq!(
        view::pane_at(&session.state, SIZE, 5, 23),
        None,
        "the footer is not a pane"
    );
}

#[test]
fn no_click_or_scroll_panics_however_small_the_terminal_or_whatever_prompt_is_open() {
    let backend = backend(8);
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    let prompts: [&[Key]; 4] = [&[], &[Key::Char('c')], &[Key::Char('?')], &[Key::Char('u')]];
    for keys in prompts {
        for key in keys {
            session.press(*key);
        }
        for width in [0u16, 1, 3, 10, 40, 100] {
            for height in [0u16, 1, 2, 5, 24] {
                let size = Rect::new(0, 0, width, height);
                for col in 0..width.min(12) {
                    for row in 0..height.min(12) {
                        let _ = view::hit_test(&session.state, size, col, row);
                    }
                }
                let _ = view::pane_at(&session.state, size, 1, 1);
            }
        }
        session.press(Key::Esc);
    }
}

// ---- restoring from the TUI -----------------------------------------------------------------

fn at_first_snapshot_of_home(backend: &InMemoryBackend, context: UiContext) -> Session<'_> {
    let mut session = Session::with_context(backend, context);
    session.press(Key::Right);
    session.press(Key::Char('G')); // the oldest snapshot, #1: before the file changed
    session
}

#[test]
fn restore_asks_first_showing_what_would_change_and_changes_nothing_until_confirmed() {
    let backend = backend(3);
    let mut session = at_first_snapshot_of_home(&backend, UiContext::default());
    session.press(Key::Char('u'));
    let Some(Modal::Restore {
        area,
        stage: RestoreStage::Ready(preview),
        ..
    }) = &session.state.modal
    else {
        panic!(
            "a prompt that has looked at the changes: {:?}",
            session.state.modal
        );
    };
    assert_eq!(area, &name("home"));
    assert_eq!(preview.changes.len(), 2, "a.txt reverted, b.txt removed");
    let screen = rows(&session.state, 100, 24).join("\n");
    assert!(
        screen.contains("1 reverted, 1 removed, 0 brought back"),
        "{screen}"
    );
    assert!(
        screen.contains("/mnt/home/a.txt") && screen.contains("snapshotted first"),
        "{screen}"
    );
    assert_eq!(
        backend.read_file("home", "/mnt/home/a.txt").as_deref(),
        Some("two"),
        "nothing yet"
    );

    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
    assert_eq!(
        backend.read_file("home", "/mnt/home/a.txt").as_deref(),
        Some("two"),
        "Esc restored nothing"
    );
    assert_eq!(backend.snapshots().len(), 3, "and took no safety snapshot");
}

#[test]
fn confirming_restores_takes_the_safety_snapshot_and_shows_the_new_snapshot_in_the_list() {
    let backend = backend(3);
    let mut session = at_first_snapshot_of_home(&backend, UiContext::default());
    session.press(Key::Char('u'));
    session.press(Key::Enter);
    session.confirm();
    assert!(session.state.modal.is_none());
    assert_eq!(
        backend.read_file("home", "/mnt/home/a.txt").as_deref(),
        Some("one")
    );
    assert_eq!(backend.read_file("home", "/mnt/home/b.txt"), None);
    let note = format!("{:?}", session.state.notice);
    assert!(
        note.contains("restored home to #1") && note.contains("#4"),
        "{note}"
    );
    let screen = rows(&session.state, 100, 24).join("\n");
    assert!(
        screen.contains("before restoring #1"),
        "the safety snapshot is listed:\n{screen}"
    );
}

#[test]
fn a_restore_prompt_can_be_confirmed_by_clicking_its_button() {
    let backend = backend(3);
    let mut session = at_first_snapshot_of_home(&backend, UiContext::default());
    session.press(Key::Char('u'));
    let drawn = rows(&session.state, 100, 24);
    let (y, line) = drawn
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("[ Enter: Restore ]"))
        .expect("a Restore button");
    click(
        &mut session,
        SIZE,
        line.find("[ Enter: Restore ]").unwrap() as u16 + 4,
        y as u16,
    );
    session.confirm();
    assert_eq!(
        backend.read_file("home", "/mnt/home/a.txt").as_deref(),
        Some("one")
    );
}

#[test]
fn the_system_root_gets_the_supported_route_and_no_way_to_confirm() {
    let backend = InMemoryBackend::with_areas(&["root"]).with_root_area("root");
    backend.write_file("root", "/etc/fstab", "good");
    backend
        .create_snapshot(&name("root"), &Description::parse("base").unwrap())
        .unwrap();
    backend.write_file("root", "/etc/fstab", "broken");
    let context = UiContext {
        boot_tip: None,
        restore_route:
            "to restore the system: reboot, pick the snapshot, run limine-snapper-restore".into(),
        file: None,
        ..UiContext::default()
    };
    let mut session = Session::with_context(&backend, context);
    session.press(Key::Right);
    session.press(Key::Char('u'));
    let screen = rows(&session.state, 100, 24).join("\n");
    assert!(
        screen.contains("is the system root") && screen.contains("limine-snapper-restore"),
        "{screen}"
    );
    assert!(
        !screen.contains("[ Enter: Restore ]"),
        "no confirm button for the system root:\n{screen}"
    );
    session.press(Key::Enter); // Enter closes information, it does not confirm
    assert!(session.state.modal.is_none());
    assert_eq!(
        backend.read_file("root", "/etc/fstab").as_deref(),
        Some("broken")
    );
    assert_eq!(
        backend.snapshots().len(),
        1,
        "no safety snapshot, nothing restored"
    );
}

#[test]
fn nothing_to_restore_is_said_and_cannot_be_confirmed() {
    let backend = InMemoryBackend::with_areas(&["home"]);
    backend
        .create_snapshot(&name("home"), &Description::parse("base").unwrap())
        .unwrap();
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    session.press(Key::Char('u'));
    let screen = rows(&session.state, 100, 24).join("\n");
    assert!(screen.contains("already matches #1"), "{screen}");
    session.press(Key::Enter);
    assert!(session.state.modal.is_none());
    assert_eq!(
        backend.snapshots().len(),
        1,
        "no safety snapshot for a no-op"
    );
}

#[test]
fn an_answer_that_arrives_after_the_prompt_was_closed_is_dropped() {
    let mut state = State::default();
    update(
        &mut state,
        Msg::ChangesLoaded {
            area: name("home"),
            id: snapall_core::SnapshotId::new(1).unwrap(),
            result: Err("late".into()),
        },
    );
    assert!(
        state.modal.is_none(),
        "no prompt springs up from a stale answer"
    );
}

#[test]
fn restore_needs_the_snapshots_pane_and_a_snapshot() {
    let backend = backend(3);
    let mut session = Session::new(&backend);
    session.press(Key::Char('u')); // focus is on the configs pane
    assert!(session.state.modal.is_none());
    assert!(
        format!("{:?}", session.state.notice).contains("snapshots pane"),
        "{:?}",
        session.state.notice
    );
}

#[test]
fn the_boot_tip_is_shown_until_a_real_notice_needs_the_line_and_returns_after() {
    let backend = backend(3);
    let tip = "tip: with limine-snapper-sync installed, snapshots appear in the boot menu";
    let context = UiContext {
        boot_tip: Some(tip.into()),
        restore_route: String::new(),
        file: None,
        ..UiContext::default()
    };
    let mut session = Session::with_context(&backend, context);
    let screen = |s: &Session<'_>| rows(&s.state, 120, 24).join("\n");
    assert!(
        screen(&session).contains("limine-snapper-sync"),
        "shown at the start"
    );

    session.state.notice = Some(snapall_tui::model::Notice::Info("created home #4".into()));
    let with_notice = screen(&session);
    assert!(
        with_notice.contains("created home #4") && !with_notice.contains("limine-snapper-sync"),
        "{with_notice}"
    );

    session.state.notice = None;
    assert!(
        screen(&session).contains("limine-snapper-sync"),
        "and back when nothing else is said"
    );
}
