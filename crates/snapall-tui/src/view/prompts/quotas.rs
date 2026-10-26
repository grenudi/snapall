// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asking before the disk starts, or stops, counting the room snapshots take: what it gives, what it
//! costs, what will happen, and that it can be undone.

use crate::view::*;
use snapall_core::explain::room_counting_text;
use snapall_core::QuotaMode;

pub(crate) fn quotas_spec(turn_on: bool, state: &State) -> ModalSpec {
    let areas: Vec<String> = state
        .health
        .data
        .as_ref()
        .map(|data| data.facts.areas.iter().map(|a| a.title()).collect())
        .unwrap_or_default();
    let mode = state
        .health
        .data
        .as_ref()
        .and_then(|data| data.facts.areas.first().map(|a| a.quotas))
        .unwrap_or(QuotaMode::Unknown);
    let will = if turn_on {
        format!(
            "snapall will switch counting on for the disk{} holding {}. The disk then builds its tally in the background; figures appear as it finishes. You can switch it off again here.",
            if areas.len() == 1 { "" } else { "s" },
            areas.join(", ")
        )
    } else {
        format!(
            "snapall will switch counting off for the disk{} holding {}. The figures go; nothing else changes. You can switch it on again here.",
            if areas.len() == 1 { "" } else { "s" },
            areas.join(", ")
        )
    };
    let lines = vec![
        Line::from(room_counting_text(mode)),
        Line::from(""),
        Line::styled(will, Style::new().fg(Color::Yellow)),
    ];
    let height = (rows_needed(&lines, 80) + 4).clamp(10, 21);
    ModalSpec {
        title: if turn_on {
            " Count the room snapshots take? ".into()
        } else {
            " Stop counting the room snapshots take? ".into()
        },
        lines,
        color: Color::Yellow,
        width: 80,
        height,
        buttons: vec![
            confirm_button(if turn_on { "Switch on" } else { "Switch off" }),
            cancel_button(),
        ],
    }
}
