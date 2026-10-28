// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The question before an area stops being protected: the plan in plain words, in red where it is
//! destructive, and the name to type.

use crate::model::StopProtecting;
use crate::view::*;
use snapall_core::Weight;

pub(crate) fn unprotect_spec(prompt: &StopProtecting) -> ModalSpec {
    let mut lines = Vec::new();
    match &prompt.plan {
        None => lines.push(Line::styled(
            "Working out what that would do...",
            Style::new().fg(DIM),
        )),
        Some(Err(why)) => {
            lines.push(Line::styled(why.clone(), Style::new().fg(Color::Yellow)));
        }
        Some(Ok(plan)) => {
            for (index, sentence) in plan.words().into_iter().enumerate() {
                // the first says what is lost, the last that it cannot be undone: both are the point
                let style = if index == 0 {
                    Style::new().fg(Color::Red).bold()
                } else {
                    Style::new()
                };
                lines.push(Line::styled(sentence, style));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(vec![
                Span::raw(format!(
                    "{}: ",
                    Weight::Destroys.instruction(Some(prompt.area.as_str()))
                )),
                Span::styled(prompt.typed.clone(), Style::new().bold()),
                Span::styled(
                    "_",
                    Style::new().fg(ACCENT).add_modifier(Modifier::SLOW_BLINK),
                ),
            ]));
            if let Some(error) = &prompt.error {
                lines.push(Line::styled(error.clone(), Style::new().fg(Color::Red)));
            }
        }
    }
    let width = 78;
    let height = (rows_needed(&lines, width) + 4).clamp(9, MAX_PROMPT_HEIGHT);
    let buttons = match &prompt.plan {
        Some(Ok(_)) => vec![confirm_button("Stop protecting"), cancel_button()],
        _ => vec![close_button()],
    };
    ModalSpec {
        title: format!(" Stop protecting {}? ", prompt.area),
        lines,
        color: Color::Red,
        width,
        height,
        buttons,
    }
}
