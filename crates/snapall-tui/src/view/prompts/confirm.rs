// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The question before a change, in the colour of its weight: yellow for what can be put back, red for what
//! cannot.

use crate::model::Confirm;
use crate::view::*;
use snapall_core::Weight;

pub(crate) fn confirm_spec(question: &Confirm) -> ModalSpec {
    let weight = question.act.weight();
    let colour = if weight == Weight::Destroys {
        Color::Red
    } else {
        Color::Yellow
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (index, sentence) in question.lines.iter().enumerate() {
        let style = if index == 0 {
            Style::new().bold()
        } else {
            Style::new()
        };
        lines.push(Line::styled(sentence.clone(), style));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        weight.instruction(question.target.as_deref()),
        Style::new().fg(colour).bold(),
    ));
    lines.push(Line::from(vec![
        Span::styled("> ", Style::new().fg(colour)),
        Span::styled(question.typed.clone(), Style::new().bold()),
        Span::styled(
            "_",
            Style::new().fg(ACCENT).add_modifier(Modifier::SLOW_BLINK),
        ),
    ]));
    if let Some(error) = &question.error {
        lines.push(Line::styled(
            format!("Not that: {error}"),
            Style::new().fg(Color::Red),
        ));
    }
    let width = 76;
    let height = (rows_needed(&lines, width) + 4).clamp(9, MAX_PROMPT_HEIGHT);
    ModalSpec {
        title: format!(" {} ", question.title),
        lines,
        color: colour,
        width,
        height,
        buttons: vec![confirm_button("Go ahead"), cancel_button()],
    }
}
