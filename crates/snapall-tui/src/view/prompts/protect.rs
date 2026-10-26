// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The offer to protect a part of the disk.

use crate::view::*;

/// The offer to protect a part of the disk: one at a time, with what it holds and what will happen.
pub(crate) fn protect_spec(index: usize, state: &State) -> ModalSpec {
    let count = state.offers.len();
    let Some(offer) = state.offers.get(index) else {
        return ModalSpec {
            title: " Protect ".into(),
            lines: vec![
                Line::from("Everything snapall recommends protecting has snapshots already."),
                Line::from(""),
                Line::from(explain::NOT_A_BACKUP),
            ],
            color: ACCENT,
            width: 72,
            height: 10,
            buttons: vec![close_button()],
        };
    };
    let mut lines: Vec<Line> = offer
        .summary
        .lines()
        .map(|l| Line::from(l.to_owned()))
        .collect();
    let mut buttons = Vec::new();
    if state.context.administrator {
        buttons.push(confirm_button("Protect it"));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "To protect it, close this and start snapall with administrator rights:",
            Style::new().fg(Color::Yellow),
        ));
        lines.push(Line::styled(
            "    sudo snapall tui",
            Style::new().fg(Color::Yellow).bold(),
        ));
    }
    if count > 1 {
        buttons.push((Button::Next, "Tab: Next".to_owned()));
    }
    buttons.push(if state.context.administrator {
        cancel_button()
    } else {
        close_button()
    });
    ModalSpec {
        title: format!(" Protect  ({} of {count}) ", index + 1),
        lines,
        color: Color::Yellow,
        width: 78,
        height: 20,
        buttons,
    }
}
