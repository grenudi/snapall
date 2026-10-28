// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Changes of several areas run in a process of their own: the window hands them over, follows them
//! through the journal, shows what is running, and can ask one to stop.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{
    plan_snapshots, run_plan, Description, JournalEvent, JournalOutcome, SnapshotBackend, Source,
    SystemProbe,
};
use snapall_tui::model::{Key, Modal, Msg};
use snapall_tui::{view, Session, Tab, UiContext};

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

fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_areas(&["home", "root"]).with_root_area("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    run_plan(&backend, &plan, &Description::parse("first").unwrap(), None);
    backend
}

fn system() -> FakeSystem {
    FakeSystem::new()
        .at("2026-10-25 12:00:00")
        .running_in_the_background()
}

/// A second of the clock: what the window does once a second while something runs.
fn a_second(session: &mut Session<'_>) {
    for _ in 0..10 {
        session.dispatch(Msg::Tick);
    }
}

/// The part of the worker: what a real `snapall create --job ID` writes to the journal.
fn worker_starts(system: &FakeSystem, id: &str) {
    system.record(&JournalEvent::Started {
        id: id.into(),
        at: "2026-10-25 12:00:01".into(),
        what: "Took a snapshot of home, root".into(),
        source: Source::Window,
        pid: 4242,
    });
}

fn worker_steps(system: &FakeSystem, id: &str, done: u32, area: &str) {
    system.record(&JournalEvent::Progress {
        id: id.into(),
        at: "2026-10-25 12:00:02".into(),
        done,
        of: 2,
        area: area.into(),
    });
}

fn worker_ends(system: &FakeSystem, id: &str, outcome: JournalOutcome) {
    system.record(&JournalEvent::Finished {
        id: id.into(),
        at: "2026-10-25 12:00:05".into(),
        outcome,
        took_ms: 4000,
    });
}

fn snapshots_of_home(backend: &InMemoryBackend) -> usize {
    backend
        .list_snapshots(&snapall_core::AreaName::parse("home").unwrap())
        .unwrap()
        .len()
}

/// A window that has asked for a snapshot of everything, which was handed over; and the change's name.
fn handed_over<'a>(backend: &'a InMemoryBackend, system: &'a FakeSystem) -> (Session<'a>, String) {
    let mut session = Session::with_system(backend, system, UiContext::default());
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    let handed = system.handed_to_the_background();
    assert_eq!(handed.len(), 1, "{handed:?}");
    (session, handed[0].0.clone())
}

#[test]
fn a_snapshot_of_several_areas_is_handed_over_as_the_ordinary_command_and_not_done_here() {
    let (backend, system) = (machine(), system());
    let before = snapshots_of_home(&backend);
    let (session, id) = handed_over(&backend, &system);
    let (_, arguments) = &system.handed_to_the_background()[0];
    assert_eq!(
        &arguments[..3],
        ["create", "--only", "home,root"],
        "{arguments:?}"
    );
    assert_eq!(
        snapshots_of_home(&backend),
        before,
        "the window did not take it itself"
    );
    assert!(
        system.events().is_empty(),
        "the process that does it writes the journal, not the window"
    );
    assert_eq!(session.state.watching.len(), 1);
    assert_eq!(session.state.watching[0].id, id);
    let notice = format!("{:?}", session.state.notice);
    assert!(
        notice.contains("started in the background")
            && notice.contains("goes on if you close the window"),
        "{notice}"
    );
    assert_eq!(
        session.state.pending, 0,
        "handing it over is not work that blocks the window"
    );
}

#[test]
fn what_runs_is_followed_through_the_journal_with_its_progress_and_shown_on_the_status_bar() {
    let (backend, system) = (machine(), system());
    let (mut session, id) = handed_over(&backend, &system);
    worker_starts(&system, &id);
    worker_steps(&system, &id, 1, "root");
    a_second(&mut session);

    let bar = rows(&session)[20].clone();
    assert!(bar.contains("1 running"), "the status bar says so: {bar}");
    session.press(Key::Char('3'));
    let text = flat(&session);
    assert!(
        text.contains("running") && text.contains("Took a snapshot of home, root (2 of 2: root)"),
        "{text}"
    );
    assert!(
        text.contains("It is being done right now by process 4242")
            && text.contains("Closing this window does not stop it"),
        "{text}"
    );
}

#[test]
fn when_it_ends_the_window_says_so_and_looks_at_the_lists_again() {
    let (backend, system) = (machine(), system());
    let (mut session, id) = handed_over(&backend, &system);
    worker_starts(&system, &id);
    a_second(&mut session);
    assert_eq!(
        session.state.watching.len(),
        1,
        "still running, still followed"
    );

    worker_ends(
        &system,
        &id,
        JournalOutcome::Done("2 snapshots taken".into()),
    );
    a_second(&mut session);
    assert!(
        session.state.watching.is_empty(),
        "no longer followed once it has ended"
    );
    assert!(format!("{:?}", session.state.notice).contains("Took a snapshot of home, root: done"));
    assert!(
        !rows(&session)[20].contains("running"),
        "the item leaves the status bar"
    );
}

#[test]
fn a_failure_in_the_background_is_said_with_its_reason() {
    let (backend, system) = (machine(), system());
    let (mut session, id) = handed_over(&backend, &system);
    worker_starts(&system, &id);
    worker_ends(
        &system,
        &id,
        JournalOutcome::Failed("home: snapper said no".into()),
    );
    a_second(&mut session);
    let notice = format!("{:?}", session.state.notice);
    assert!(
        notice.contains("Error") && notice.contains("home: snapper said no"),
        "{notice}"
    );
}

#[test]
fn a_click_on_the_running_item_goes_to_the_running_change() {
    let (backend, system) = (machine(), system());
    let (mut session, id) = handed_over(&backend, &system);
    worker_starts(&system, &id);
    a_second(&mut session);
    let col = rows(&session)[20].find("1 running").unwrap();
    let click =
        view::hit_test(&session.state, SCREEN, u16::try_from(col + 2).unwrap(), 20).unwrap();
    session.dispatch(Msg::Click(click));
    assert_eq!(session.state.tab, Tab::Activity);
    assert_eq!(
        session.state.activity.entries[session.state.activity.cursor]
            .id
            .as_deref(),
        Some(id.as_str())
    );
}

#[test]
fn stopping_asks_first_says_what_it_means_and_only_asks() {
    let (backend, system) = (machine(), system());
    let (mut session, id) = handed_over(&backend, &system);
    worker_starts(&system, &id);
    a_second(&mut session);
    session.press(Key::Char('3'));

    session.press(Key::Char('x'));
    assert!(
        matches!(&session.state.modal, Some(Modal::StopChange { id: asked, .. }) if *asked == id)
    );
    let text = flat(&session);
    for must in [
        "Stop it after this area?",
        "finishes the area it is on, and then stops",
        "never stopped in the middle of an area",
        "Closing this window does not stop it",
    ] {
        assert!(text.contains(must), "`{must}` missing:\n{text}");
    }
    let screen = rows(&session);
    assert!(
        screen[20].contains("running") && screen[23].contains("q quit"),
        "the bars are not covered"
    );
    session.press(Key::Esc);
    assert!(system.stops_asked().is_empty(), "leaving asks nothing");

    session.press(Key::Char('x'));
    session.press(Key::Enter);
    session.confirm();
    assert_eq!(system.stops_asked(), std::slice::from_ref(&id));
    assert!(
        format!("{:?}", session.state.notice).contains("it finishes the area it is on, then stops")
    );
}

#[test]
fn x_on_a_change_that_is_not_running_says_what_it_is_for_and_asks_nothing() {
    let (backend, system) = (machine(), system());
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    system.record(&JournalEvent::Started {
        id: "old".into(),
        at: "2026-10-25 09:00:00".into(),
        what: "Took a snapshot of home".into(),
        source: Source::Command,
        pid: 1,
    });
    worker_ends(&system, "old", JournalOutcome::Done(String::new()));
    session.press(Key::Char('3'));
    session.press(Key::Char('x'));
    assert!(session.state.modal.is_none());
    assert!(format!("{:?}", session.state.notice).contains("this one is not running"));
    assert!(system.stops_asked().is_empty());
}

#[test]
fn a_change_that_never_reports_is_let_go_of_with_a_word_about_it() {
    let (backend, system) = (machine(), system());
    let (mut session, _) = handed_over(&backend, &system);
    for _ in 0..40 {
        a_second(&mut session);
    }
    assert!(
        session.state.watching.is_empty(),
        "it is not followed for ever"
    );
    assert!(format!("{:?}", session.state.notice).contains("never reported"));
}

#[test]
fn a_machine_that_cannot_run_in_the_background_does_the_change_itself_as_before() {
    let (backend, system) = (machine(), FakeSystem::new().at("2026-10-25 12:00:00"));
    let before = snapshots_of_home(&backend);
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    session.confirm();
    assert!(system.handed_to_the_background().is_empty());
    assert_eq!(snapshots_of_home(&backend), before + 1, "taken here");
    assert_eq!(
        system.events().len(),
        3,
        "and written to the journal by the window: its start, the way back from it, its end"
    );
}

#[test]
fn a_machine_that_refuses_to_start_it_falls_back_to_doing_it_here() {
    let (backend, system) = (machine(), system().refusing_jobs());
    let before = snapshots_of_home(&backend);
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    assert_eq!(snapshots_of_home(&backend), before + 1, "it still happens");
}
