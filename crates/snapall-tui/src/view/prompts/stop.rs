// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The question before a running change is asked to stop.

use crate::view::*;

pub(crate) fn stop_spec(what: &str) -> ModalSpec {
    let lines = vec![
        Line::from(format!("Ask this to stop?  {what}")),
        Line::from(""),
        Line::from("It finishes the area it is on, and then stops. It is never stopped in the middle of an area, so nothing is left half-done, and the areas already done stay done."),
        Line::from(""),
        Line::styled("Closing this window does not stop it; only this does. It stops within moments once the area it is on is finished.", Style::new().fg(DIM)),
    ];
    let height = (rows_needed(&lines, 76) + 4).clamp(9, 21);
    ModalSpec {
        title: " Stop it after this area? ".into(),
        lines,
        color: Color::Yellow,
        width: 76,
        height,
        buttons: vec![confirm_button("Ask it to stop"), cancel_button()],
    }
}
