// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The prompt for a snapshot's note.

use crate::view::*;

/// The prompt for a new snapshot's description.
pub(crate) fn describe_spec(
    target: &Target,
    input: &str,
    error: Option<&str>,
    state: &State,
) -> ModalSpec {
    let title = match target {
        Target::One(config) => format!(" New snapshot of {config} "),
        Target::All => {
            let names: Vec<String> = state.entries().iter().map(|e| e.name.to_string()).collect();
            format!(
                " New snapshot of ALL protected areas ({}) ",
                names.join(", ")
            )
        }
        Target::Group(name) => {
            let areas = state.groups.get(name).unwrap_or(&[]);
            let list: Vec<String> = areas.iter().map(ToString::to_string).collect();
            format!(" New snapshot of the group {name} ({}) ", list.join(", "))
        }
    };
    let mut lines = vec![
        Line::from("A short note to help you recognise this snapshot later (Enter alone uses \"snapall\"):"),
        Line::from(vec![
            Span::styled(input.to_owned(), Style::new().bold()),
            Span::styled(
                "_",
                Style::new().fg(ACCENT).add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
    ];
    if let Some(error) = error {
        lines.push(Line::styled(error.to_owned(), Style::new().fg(Color::Red)));
    }
    // several areas can be narrowed to a group, if there are any: the way to do it is on the screen
    if !matches!(target, Target::One(_)) && !state.groups.is_empty() {
        lines.push(Line::styled(
            "Tab: all the areas, or one of your groups",
            Style::new().fg(DIM),
        ));
    }
    let height = (rows_needed(&lines, 64) + 4).clamp(9, 21);
    ModalSpec {
        title,
        lines,
        color: ACCENT,
        width: 64,
        height,
        buttons: vec![confirm_button("Create"), cancel_button()],
    }
}
