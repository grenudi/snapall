// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asking before the disk starts, or stops, counting the room snapshots take.

use crate::model::*;
use snapall_core::Act;

/// Enter or y does it, Esc leaves. Switching it needs administrator rights, and says so rather than failing.
pub(crate) fn confirming_quotas(state: &mut State, key: Key, turn_on: bool) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q' | 'n' | 'N') => Reaction::closes(),
        Key::Enter | Key::Char('y' | 'Y') if !state.context.administrator => {
            state.notice = Some(Notice::Info(
                "Changing how room is counted needs administrator rights: start snapall with sudo."
                    .into(),
            ));
            Reaction::closes()
        }
        Key::Enter | Key::Char('y' | 'Y') => {
            let paths: Vec<String> = state
                .health
                .data
                .as_ref()
                .map(|data| {
                    data.facts
                        .areas
                        .iter()
                        .map(|area| area.subvolume.clone())
                        .collect()
                })
                .unwrap_or_default();
            let lines = vec![
                format!(
                    "Switch counting the room snapshots take {} for the whole disk.",
                    if turn_on { "on" } else { "off" }
                ),
                "It can be switched back the same way.".to_owned(),
            ];
            ask_first(
                Act::RoomCounting,
                None,
                lines,
                vec![Effect::SetQuotas { turn_on, paths }],
            )
        }
        _ => Reaction::stays(Modal::Quotas { turn_on }),
    }
}
