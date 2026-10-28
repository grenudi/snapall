#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Work in flight: what the model counts, what it refuses, and what stays available while
//! snapper is busy. The pure model is driven directly (no executor), so "in flight" can be held
//! open as long as a test needs.

use snapall_core::AreaName;
use snapall_tui::model::{update, Effect, Key, Modal, Msg, Notice, State};

fn name(s: &str) -> AreaName {
    AreaName::parse(s).unwrap()
}

/// A state with one area selected and its snapshots still being fetched: one effect in flight.
fn busy() -> State {
    let mut state = State::default();
    update(&mut state, Msg::Start);
    update(&mut state, Msg::HealthLoaded(Err(String::new())));
    let effects = update(&mut state, Msg::AreasLoaded(Ok(vec![name("home")])));
    assert_eq!(effects, [Effect::LoadSnapshots(name("home"))]);
    assert_eq!(state.pending, 1);
    state
}

fn press(state: &mut State, key: Key) -> Vec<Effect> {
    update(state, Msg::Key(key))
}

#[test]
fn starting_asks_for_the_areas_first_and_how_protection_stands_only_after_the_first_screen_has_its_data(
) {
    let mut state = State::default();
    assert_eq!(update(&mut state, Msg::Start), [Effect::LoadAreas]);
    // the areas are work in flight; the health report loads on the side and does not count
    assert_eq!(state.pending, 1);
    update(&mut state, Msg::HealthLoaded(Err("not measured".into())));
    assert_eq!(
        state.pending, 1,
        "its arrival settles nothing, it was never counted"
    );
}

#[test]
fn a_result_settles_one_pending_effect_and_may_ask_for_more() {
    let mut state = State::default();
    update(&mut state, Msg::Start);
    update(&mut state, Msg::HealthLoaded(Err(String::new())));
    assert_eq!(state.pending, 1);
    // The area list came back (-1) and asks for the first area's snapshots (+1).
    update(&mut state, Msg::AreasLoaded(Ok(vec![name("home")])));
    assert_eq!(state.pending, 1);
    // The snapshots came back (-1) and ask for nothing more.
    update(
        &mut state,
        Msg::SnapshotsLoaded {
            area: name("home"),
            result: Ok(Vec::new()),
        },
    );
    assert_eq!(state.pending, 0);
}

#[test]
fn a_result_nobody_asked_for_never_takes_the_count_below_zero() {
    let mut state = State::default();
    update(&mut state, Msg::AreasLoaded(Ok(Vec::new())));
    assert_eq!(state.pending, 0);
}

#[test]
fn starting_more_work_is_refused_while_snapper_is_busy() {
    for key in ['c', 'a', 'd', 'r'] {
        let mut state = busy();
        let effects = press(&mut state, Key::Char(key));
        assert!(effects.is_empty(), "{key}: {effects:?}");
        assert!(state.modal.is_none(), "{key} must not open a prompt");
        assert_eq!(state.pending, 1, "{key} must not start work");
        assert!(
            matches!(&state.notice, Some(Notice::Error(t)) if t.contains("busy")),
            "{key}: {:?}",
            state.notice
        );
    }
}

#[test]
fn moving_and_reading_stay_available_while_snapper_is_busy() {
    let mut state = busy();
    press(&mut state, Key::Right);
    press(&mut state, Key::Char('j'));
    press(&mut state, Key::Tab);
    assert_eq!(state.pending, 1, "navigation starts no work");
    press(&mut state, Key::Char('?'));
    assert_eq!(state.modal, Some(Modal::Help { page: 0 }), "help opens");
}

#[test]
fn work_can_be_started_again_once_the_last_result_is_in() {
    let mut state = busy();
    update(
        &mut state,
        Msg::SnapshotsLoaded {
            area: name("home"),
            result: Ok(Vec::new()),
        },
    );
    assert_eq!(state.pending, 0);
    press(&mut state, Key::Char('c'));
    assert!(matches!(state.modal, Some(Modal::Describe { .. })));
}

#[test]
fn a_tick_animates_the_indicator_and_changes_nothing_else() {
    let mut state = busy();
    let before = (state.pending, state.spinner);
    assert!(update(&mut state, Msg::Tick).is_empty());
    assert_eq!(state.pending, before.0);
    assert_eq!(state.spinner, before.1 + 1);
}

#[test]
fn quitting_while_busy_is_recorded_and_the_work_in_flight_is_still_counted() {
    // The driver, not the model, waits for pending to reach zero before leaving.
    let mut state = busy();
    press(&mut state, Key::Char('q'));
    assert!(state.quit);
    assert_eq!(state.pending, 1);
}

#[test]
fn a_prompt_opened_before_the_work_began_can_still_be_cancelled() {
    let mut state = State::default();
    update(&mut state, Msg::Start);
    update(&mut state, Msg::HealthLoaded(Err(String::new())));
    update(&mut state, Msg::AreasLoaded(Ok(vec![name("home")])));
    update(
        &mut state,
        Msg::SnapshotsLoaded {
            area: name("home"),
            result: Ok(Vec::new()),
        },
    );
    press(&mut state, Key::Char('c'));
    assert!(state.modal.is_some());
    press(&mut state, Key::Esc);
    assert!(state.modal.is_none());
    assert_eq!(state.pending, 0);
}

#[test]
fn the_health_report_is_asked_for_after_the_first_screen_and_never_blocks_the_keys_that_start_work()
{
    // The worker does one thing at a time, so a status refresh queued before the first screen's data
    // would delay it: the report is asked for only once that data has arrived.
    let mut state = State::default();
    assert_eq!(update(&mut state, Msg::Start), [Effect::LoadAreas]);
    let effects = update(&mut state, Msg::AreasLoaded(Ok(vec![])));
    assert_eq!(
        effects,
        [Effect::LoadHealth, Effect::LoadHistory, Effect::LoadGroups]
    );
    assert!(
        update(&mut state, Msg::AreasLoaded(Ok(vec![]))).is_empty(),
        "and only once"
    );
    // it is on its way, and it is not work: nothing that starts work is refused for it
    assert_eq!(state.pending, 0);
    let refused = press(&mut state, Key::Char('r'));
    assert!(
        !format!("{:?}", state.notice).contains("busy"),
        "{:?} {refused:?}",
        state.notice
    );
}
