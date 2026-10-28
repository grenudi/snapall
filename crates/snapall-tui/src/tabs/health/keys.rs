// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What pressing a key does on the Health tab: moving along the rows and acting on the chosen one.

use super::*;
use crate::model::{Effect, Key, Modal, Notice, State};
use snapall_core::Remedy;

/// The keys of this tab. `None` means the key is not one of its own, and the window handles it.
pub fn on_key(state: &mut State, key: Key) -> Option<Vec<Effect>> {
    let count = state.health.rows().len();
    let last = count.saturating_sub(1);
    match key {
        Key::Up | Key::Char('k') => state.health.cursor = state.health.cursor.saturating_sub(1),
        Key::Down | Key::Char('j') => state.health.cursor = (state.health.cursor + 1).min(last),
        Key::Home => state.health.cursor = 0,
        Key::End | Key::Char('G') => state.health.cursor = last,
        Key::Enter => return Some(act_on_chosen(state)),
        // stopping protection is a key of its own, never the Enter that a row means for something milder
        Key::Char('U') => return Some(stop_protecting_the_chosen(state)),
        _ => return None,
    }
    Some(Vec::new())
}

/// `U` on an area's row opens the question before it stops being protected; on any other row it says what
/// it is for.
pub(super) fn stop_protecting_the_chosen(state: &mut State) -> Vec<Effect> {
    let area = state
        .health
        .rows()
        .into_iter()
        .nth(state.health.cursor)
        .and_then(|row| row.area);
    match area {
        Some(config) => crate::model::open_unprotect(state, config),
        None => {
            state.notice = Some(Notice::Info(
                "U stops protecting an area: choose one of the areas under Space first".into(),
            ));
            Vec::new()
        }
    }
}

/// Enter on a row: do what it offers, or open the controls it leads to.
pub(super) fn act_on_chosen(state: &mut State) -> Vec<Effect> {
    let Some(row) = state.health.rows().into_iter().nth(state.health.cursor) else {
        return Vec::new();
    };
    match row.action {
        RowAction::Nothing => Vec::new(),
        RowAction::Fix(Remedy::SwitchOnCleanup) if !state.context.administrator => {
            state.notice = Some(Notice::Info(
                "Switching cleanup on needs administrator rights: start snapall with sudo.".into(),
            ));
            Vec::new()
        }
        RowAction::Fix(Remedy::SwitchOnCleanup) => {
            let lines = vec![
                "Switch automatic cleanup on.".to_owned(),
                "Old snapshots are then removed by themselves, within the limits set for each area. It can be switched off again from the Health tab.".to_owned(),
            ];
            state.modal = Some(crate::model::confirm_modal(
                snapall_core::Act::ControlJob,
                None,
                lines,
                vec![Effect::SwitchOnCleanup],
            ));
            Vec::new()
        }
        RowAction::Fix(Remedy::CountRoom) | RowAction::CountRoom(true) => {
            state.modal = Some(Modal::Quotas { turn_on: true });
            Vec::new()
        }
        RowAction::CountRoom(false) => {
            state.modal = Some(Modal::Quotas { turn_on: false });
            Vec::new()
        }
        RowAction::EditLimits(config) => crate::model::open_limits(state, config),
        RowAction::ControlJob(job) => {
            state.modal = Some(Modal::Job {
                unit: job.unit.to_owned(),
            });
            Vec::new()
        }
    }
}
