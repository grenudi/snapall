// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The Activity tab shows what snapall did before this window too, and from the command line.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{
    plan_snapshots, run_plan, Description, JournalEvent, JournalOutcome, Source, SystemProbe,
};
use snapall_tui::model::Key;
use snapall_tui::{view, Session, UiContext};

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
    let rows: Vec<String> = (0..SCREEN.height)
        .map(|y| {
            (0..SCREEN.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect();
    rows.join(" ")
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
    let backend = InMemoryBackend::with_configs(&["home", "root"]).with_root_config("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    run_plan(&backend, &plan, &Description::parse("first").unwrap(), None);
    backend
}

fn earlier_command(id: &str, what: &str, pid: u32, day: &str) -> [JournalEvent; 1] {
    [JournalEvent::Started {
        id: id.into(),
        at: format!("{day} 09:30:00"),
        what: what.into(),
        source: Source::Command,
        pid,
    }]
}

fn ended(id: &str, day: &str, outcome: Result<&str, &str>) -> JournalEvent {
    JournalEvent::Finished {
        id: id.into(),
        at: format!("{day} 09:30:04"),
        outcome: outcome.map(str::to_owned).map_err(str::to_owned).into(),
        took_ms: 4000,
    }
}

#[test]
fn changes_from_earlier_sessions_and_from_commands_are_shown_with_their_day() {
    let mut system = FakeSystem::new().at("2026-10-25 12:00:00");
    for event in earlier_command("old-1", "Took a snapshot of home, root", 1, "2026-10-24")
        .into_iter()
        .chain([ended("old-1", "2026-10-24", Ok("2 snapshots taken"))])
    {
        system = system.with_event(event);
    }
    let backend = machine();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('3'));
    let text = screen(&session);
    assert!(
        text.contains("done Took a snapshot of home, root (command)"),
        "{text}"
    );
    assert!(
        text.contains("10-24 09:30"),
        "an earlier day is shown with its date: {text}"
    );
    assert!(text.contains("(4.0 s)"), "{text}");
}

#[test]
fn a_change_made_now_is_recorded_once_and_shown_once_whichever_way_it_is_read() {
    let system = FakeSystem::new().at("2026-10-25 12:00:00");
    let backend = machine();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('a'));
    session.press(Key::Enter);

    // a start, the way back from it, and its end
    let events = system.events();
    assert_eq!(events.len(), 3, "{events:?}");
    let (
        JournalEvent::Started {
            id: a,
            source,
            what,
            ..
        },
        JournalEvent::Finished { id: b, outcome, .. },
    ) = (&events[0], &events[2])
    else {
        panic!("a start and an end: {events:?}");
    };
    assert!(
        matches!(&events[1], JournalEvent::Rollback { id, rollback: snapall_core::Rollback::Undo(_), .. } if id == a),
        "taking a snapshot records how to undo it, on the same change: {events:?}"
    );
    assert_eq!(
        (a, *source, what.as_str()),
        (b, Source::Window, "Took a snapshot of home, root")
    );
    assert_eq!(
        outcome,
        &JournalOutcome::Done("2 snapshots taken".to_owned())
    );

    session.press(Key::Char('3'));
    session.press(Key::Char('1'));
    session.press(Key::Char('3'));
    assert_eq!(
        session.state.activity.entries.len(),
        1,
        "read three times, shown once"
    );
}

#[test]
fn a_change_that_was_cut_short_shows_as_stopped_and_says_what_to_do_next() {
    let system = FakeSystem::new().at("2026-10-25 12:00:00").with_event(
        earlier_command("cut", "Putting Home back to snapshot #3", 0, "2026-10-25")[0].clone(),
    );
    let backend = machine();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('3'));
    let text = screen(&session);
    assert!(
        text.contains("STOPPED Putting Home back to snapshot #3"),
        "{text}"
    );
    assert!(
        text.contains("stopped before it could say how it ended"),
        "{text}"
    );
    assert!(
        text.contains("Look at the snapshots to see how far it got"),
        "{text}"
    );
}

#[test]
fn a_failure_keeps_its_reason_in_the_record() {
    let system = FakeSystem::new().at("2026-10-25 12:00:00");
    let backend = machine();
    backend.refuse_snapshots_of("home");
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    session.press(Key::Char('3'));
    let text = screen(&session);
    assert!(
        text.contains("FAILED Took a snapshot of home, root"),
        "{text}"
    );
    assert!(text.contains("It failed: home:"), "{text}");
    assert!(
        system.events().iter().any(|e| matches!(
            e,
            JournalEvent::Finished {
                outcome: JournalOutcome::Failed(_),
                ..
            }
        )),
        "the failure is in the journal too"
    );
}

#[test]
fn what_another_snapall_did_while_this_window_was_open_appears_when_the_tab_is_opened() {
    let system = FakeSystem::new().at("2026-10-25 12:00:00");
    let backend = machine();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('3'));
    assert!(screen(&session).contains("Nothing yet"));
    // a command, in another terminal, does something
    system.record(&earlier_command("other", "Deleted 1 snapshot of home", 1, "2026-10-25")[0]);
    system.record(&ended("other", "2026-10-25", Ok("")));
    session.press(Key::Char('1'));
    session.press(Key::Char('3'));
    assert!(screen(&session).contains("done Deleted 1 snapshot of home (command)"));
}

#[test]
fn where_nothing_is_remembered_the_window_still_shows_what_it_just_did() {
    let backend = machine();
    let mut session = Session::new(&backend); // a machine with no journal
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    session.press(Key::Char('3'));
    assert!(screen(&session).contains("Took a snapshot of home, root"));
    session.press(Key::Char('1'));
    session.press(Key::Char('3'));
    assert_eq!(session.state.activity.entries.len(), 1, "still once");
}

#[test]
fn a_change_refused_because_another_snapall_is_busy_is_shown_with_who_and_leaves_no_false_record() {
    let system = FakeSystem::new()
        .at("2026-10-25 12:00:00")
        .busy_with("restoring Home");
    let backend = machine();
    let mut session = Session::with_system(&backend, &system, UiContext::default());
    session.press(Key::Char('a'));
    session.press(Key::Enter);
    assert!(
        system.events().is_empty(),
        "nothing was started, so nothing is journaled as started"
    );
    session.press(Key::Char('3'));
    let text = screen(&session);
    assert!(
        text.contains("FAILED Took a snapshot of home, root")
            && text.contains("Another snapall is making a change right now (restoring Home"),
        "{text}"
    );
}
