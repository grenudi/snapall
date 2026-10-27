// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The controls of a background job and the question about counting room, in the Health tab.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use snapall_core::services::{self, JobAction};
use snapall_core::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{plan_snapshots, run_plan, Description, QuotaMode, SystemProbe};
use snapall_tui::model::{Key, Modal, Msg};
use snapall_tui::tabs::health::RowAction;
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

fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_configs(&["home", "root"]).with_root_config("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    run_plan(&backend, &plan, &Description::parse("first").unwrap(), None);
    backend
}

fn system() -> FakeSystem {
    FakeSystem::new()
        .with_disk(100, 40)
        .at("2026-01-02 12:00:00")
        .with_service(services::CLEANUP.unit, "disabled", "inactive")
        .with_unit_file(
            services::CLEANUP.unit,
            "Daily Cleanup of Snapper Snapshots",
            &[("OnBootSec", "10m"), ("OnUnitActiveSec", "1d")],
        )
}

fn admin() -> UiContext {
    UiContext {
        administrator: true,
        ..UiContext::default()
    }
}

/// Go to the Health tab and choose the row whose Enter does `wanted`.
fn choose(session: &mut Session<'_>, wanted: impl Fn(&RowAction) -> bool) {
    session.press(Key::Char('2'));
    let at = session
        .state
        .health
        .rows()
        .iter()
        .position(|row| wanted(&row.action))
        .expect("such a row");
    session.state.health.cursor = at;
}

fn the_cleanup_job(action: &RowAction) -> bool {
    matches!(action, RowAction::ControlJob(job) if job.unit == services::CLEANUP.unit)
}

#[test]
fn a_jobs_controls_show_its_real_name_its_own_words_its_schedule_and_every_command_made_from_the_name(
) {
    let (backend, machine) = (machine(), system());
    let mut session = Session::with_system(&backend, &machine, admin());
    choose(&mut session, the_cleanup_job);
    session.press(Key::Enter);
    assert!(
        matches!(&session.state.modal, Some(Modal::Job { unit }) if unit == services::CLEANUP.unit)
    );
    let text = flat(&screen(&session));
    for must in [
        "Cleaning up old snapshots",
        "snapper-cleanup.timer",
        "Daily Cleanup of Snapper Snapshots",
        "10 minutes after the computer starts, then again 1 day after each run starts",
        "disabled at start, inactive at the moment",
        // each control beside the command that does it, all made from the unit name
        "sudo systemctl enable --now snapper-cleanup.timer",
        "sudo systemctl disable --now snapper-cleanup.timer",
        "sudo systemctl start snapper-cleanup.timer",
        "sudo systemctl stop snapper-cleanup.timer",
        "sudo systemctl restart snapper-cleanup.timer",
        "sudo systemctl start snapper-cleanup.service", // run the work now
        "systemctl status snapper-cleanup.timer",
        "journalctl -u snapper-cleanup.service -n 50",
    ] {
        assert!(
            text.contains(must),
            "`{must}` missing at 80x24:\n{}",
            screen(&session)
        );
    }
}

#[test]
fn the_keys_do_what_they_say_and_the_report_follows() {
    let (backend, machine) = (machine(), system());
    let mut session = Session::with_system(&backend, &machine, admin());
    choose(&mut session, the_cleanup_job);
    session.press(Key::Enter);
    session.press(Key::Char('e'));
    assert!(session.state.modal.is_none(), "it closed");
    assert_eq!(
        machine.job_actions(),
        [(services::CLEANUP.unit.to_owned(), JobAction::EnableNow)]
    );
    assert!(
        flat(&screen(&session)).contains("cleanup on"),
        "the status bar turned"
    );

    // 'n' runs the work of a timer right now: the service, not the timer
    choose(&mut session, the_cleanup_job);
    session.press(Key::Enter);
    session.press(Key::Char('n'));
    assert_eq!(
        machine.job_actions().last().unwrap(),
        &("snapper-cleanup.service".to_owned(), JobAction::Start)
    );
    // and the other keys act on the job itself
    for (key, action) in [
        ('x', JobAction::Stop),
        ('r', JobAction::Restart),
        ('s', JobAction::Start),
        ('d', JobAction::DisableNow),
    ] {
        choose(&mut session, the_cleanup_job);
        session.press(Key::Enter);
        session.press(Key::Char(key));
        assert_eq!(
            machine.job_actions().last().unwrap(),
            &(services::CLEANUP.unit.to_owned(), action),
            "{key}"
        );
    }
}

#[test]
fn without_administrator_rights_a_job_key_explains_and_keeps_the_prompt_open() {
    let (backend, machine) = (machine(), system());
    let mut session = Session::with_system(&backend, &machine, UiContext::default());
    choose(&mut session, the_cleanup_job);
    session.press(Key::Enter);
    session.press(Key::Char('e'));
    assert!(machine.job_actions().is_empty());
    assert!(
        matches!(session.state.modal, Some(Modal::Job { .. })),
        "still open, so the commands can be read"
    );
    assert!(format!("{:?}", session.state.notice).contains("needs administrator rights"));
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());
}

#[test]
fn a_machine_that_refuses_says_why_in_the_activity_record() {
    let (backend, machine) = (machine(), system().refusing_jobs());
    let mut session = Session::with_system(&backend, &machine, admin());
    choose(&mut session, the_cleanup_job);
    session.press(Key::Enter);
    session.press(Key::Char('e'));
    assert!(format!("{:?}", session.state.notice)
        .contains("Could not do that to Cleaning up old snapshots: Access denied"));
    session.press(Key::Char('3'));
    let text = flat(&screen(&session));
    assert!(
        text.contains("FAILED Switched on Cleaning up old snapshots")
            && text.contains("It failed: Access denied"),
        "{text}"
    );
}

#[test]
fn counting_room_is_explained_with_its_price_asked_about_and_can_be_switched_both_ways() {
    let (backend, machine) = (machine(), system());
    let mut session = Session::with_system(&backend, &machine, admin());
    choose(&mut session, |action| {
        matches!(action, RowAction::CountRoom(true))
    });
    assert!(flat(&screen(&session)).contains("Enter: switch room counting on (it explains first)"));
    session.press(Key::Enter);
    assert!(matches!(
        session.state.modal,
        Some(Modal::Quotas { turn_on: true })
    ));
    let text = flat(&screen(&session));
    for must in [
        "The price",
        "slow some operations down",
        "deleting snapshots most of all",
        "whole disk, not to one area",
        "leaving it off is the better choice",
        "snapall will switch counting on for the disk",
        "[ Enter: Switch on ]",
    ] {
        assert!(
            text.contains(must),
            "`{must}` missing at 80x24:\n{}",
            screen(&session)
        );
    }
    session.press(Key::Esc);
    assert_eq!(
        machine.quota_mode("/"),
        QuotaMode::Off,
        "cancelling changes nothing"
    );

    choose(&mut session, |action| {
        matches!(action, RowAction::CountRoom(true))
    });
    session.press(Key::Enter);
    session.press(Key::Enter);
    assert_eq!(machine.quota_mode("/"), QuotaMode::Full);
    assert!(format!("{:?}", session.state.notice).contains("building its tally in the background"));

    // and back: the row now offers to switch it off, and says the cost of keeping it
    choose(&mut session, |action| {
        matches!(action, RowAction::CountRoom(false))
    });
    session.press(Key::Enter);
    assert!(flat(&screen(&session)).contains("Switching it off removes the tally"));
    session.press(Key::Enter);
    assert_eq!(machine.quota_mode("/"), QuotaMode::Off);
}

#[test]
fn the_simple_kind_is_explained_not_offered_for_a_switch() {
    let (backend, machine) = (machine(), system().with_quotas(QuotaMode::Simple));
    let mut session = Session::with_system(&backend, &machine, admin());
    session.press(Key::Char('2'));
    let row = session
        .state
        .health
        .rows()
        .into_iter()
        .find(|r| r.label.starts_with("Counting the room"))
        .unwrap();
    assert_eq!(
        row.action,
        RowAction::Nothing,
        "snapall cannot switch one kind to the other in place"
    );
    assert!(
        row.detail.contains("cannot be told from it"),
        "{}",
        row.detail
    );
}

#[test]
fn without_administrator_rights_changing_room_counting_says_so_and_changes_nothing() {
    let (backend, machine) = (machine(), system());
    let mut session = Session::with_system(&backend, &machine, UiContext::default());
    choose(&mut session, |action| {
        matches!(action, RowAction::CountRoom(true))
    });
    session.press(Key::Enter);
    session.press(Key::Enter);
    assert_eq!(machine.quota_mode("/"), QuotaMode::Off);
    assert!(format!("{:?}", session.state.notice).contains("needs administrator rights"));
}

#[test]
fn these_prompts_leave_the_status_bar_and_the_two_lines_beneath_it_and_use_no_untold_word() {
    let (backend, machine) = (machine(), system());
    let mut session = Session::with_system(&backend, &machine, admin());
    for open in [the_cleanup_job as fn(&RowAction) -> bool, |a| {
        matches!(a, RowAction::CountRoom(true))
    }] {
        choose(&mut session, open);
        session.press(Key::Enter);
        session.dispatch(Msg::Tick);
        let text = screen(&session);
        assert!(
            text.lines().nth(20).unwrap().contains("snapshots"),
            "status bar covered:\n{text}"
        );
        assert!(
            text.lines().nth(23).unwrap().contains("q quit"),
            "key line covered:\n{text}"
        );
        let lower = flat(&text).to_lowercase();
        for word in ["subvolume", "btrfs", "qgroup", "config"] {
            assert!(!lower.contains(word), "`{word}` in:\n{text}");
        }
        session.press(Key::Esc);
    }
}
