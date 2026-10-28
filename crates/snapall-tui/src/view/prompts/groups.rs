// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The groups prompt: the list, naming a new group, and choosing its areas, each with the keys that work
//! on it said on the screen.

use crate::model::{rows, GroupsScreen};
use crate::view::*;

pub(crate) fn groups_spec(screen: &GroupsScreen, state: &State) -> ModalSpec {
    let (title, mut lines) = match screen {
        GroupsScreen::List { cursor } => (" Groups of areas ", list_lines(*cursor, state)),
        GroupsScreen::Name { input, error } => {
            (" A new group ", name_lines(input, error.as_deref()))
        }
        GroupsScreen::Pick {
            name,
            picks,
            cursor,
            error,
        } => {
            return pick_spec(name.as_str(), picks, *cursor, error.as_deref());
        }
    };
    lines.push(Line::from(""));
    let help = match screen {
        GroupsScreen::List { .. } => {
            "n: a new group   Enter: change its areas   d: take it away   Esc: close"
        }
        _ => "Enter: go on   Esc: back",
    };
    lines.push(Line::styled(help, Style::new().fg(DIM)));
    let width = 78;
    let height = (rows_needed(&lines, width) + 4).clamp(9, MAX_PROMPT_HEIGHT);
    ModalSpec {
        title: title.into(),
        lines,
        color: ACCENT,
        width,
        height,
        buttons: vec![close_button()],
    }
}

fn list_lines(cursor: usize, state: &State) -> Vec<Line<'static>> {
    let protected = state.area_names();
    let mut lines = vec![
        Line::from("A group is a name for some of your areas, so that one snapshot can take just those (press a, then Tab to choose the group). It does not change the areas or their snapshots."),
        Line::from(""),
    ];
    if let Some(why) = &state.groups_error {
        lines.push(Line::styled(
            format!("The groups could not be read: {why}"),
            Style::new().fg(Color::Red),
        ));
        lines.push(Line::from(""));
    }
    for (index, row) in rows(&state.groups).into_iter().enumerate() {
        let (name, what, level) = match &row {
            None => (
                "all".to_owned(),
                format!(
                    "every protected area ({})",
                    protected
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Color::Reset,
            ),
            Some(name) => {
                let areas = state.groups.get(name).unwrap_or(&[]);
                let missing = areas.iter().any(|a| !protected.contains(a));
                let list: Vec<String> = areas
                    .iter()
                    .map(|a| {
                        if protected.contains(a) {
                            a.to_string()
                        } else {
                            format!("{a} (no longer protected)")
                        }
                    })
                    .collect();
                (
                    name.to_string(),
                    list.join(", "),
                    if missing { Color::Yellow } else { Color::Reset },
                )
            }
        };
        let marker = if index == cursor { " > " } else { "   " };
        let style = if index == cursor {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(format!("{name:<14}"), style.bold()),
            Span::styled(what, style.fg(level)),
        ]));
    }
    lines
}

fn name_lines(input: &str, error: Option<&str>) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from("What should the group be called? Lower-case letters, digits, - and _, starting with a letter."),
        Line::from(vec![
            Span::styled(input.to_owned(), Style::new().bold()),
            Span::styled("_", Style::new().fg(ACCENT).add_modifier(Modifier::SLOW_BLINK)),
        ]),
    ];
    if let Some(error) = error {
        lines.push(Line::from(""));
        lines.push(Line::styled(error.to_owned(), Style::new().fg(Color::Red)));
    }
    lines
}

fn pick_spec(
    name: &str,
    picks: &[(snapall_core::AreaName, bool)],
    cursor: usize,
    error: Option<&str>,
) -> ModalSpec {
    let mut lines = vec![
        Line::from(format!("Which areas are in {name}?")),
        Line::from(""),
    ];
    for (index, (area, ticked)) in picks.iter().enumerate() {
        let style = if index == cursor {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        lines.push(Line::from(vec![
            Span::styled(if index == cursor { " > " } else { "   " }, style),
            Span::styled(
                if *ticked { "[x] " } else { "[ ] " },
                style.fg(Color::Green),
            ),
            Span::styled(area.to_string(), style),
        ]));
    }
    if let Some(error) = error {
        lines.push(Line::from(""));
        lines.push(Line::styled(error.to_owned(), Style::new().fg(Color::Red)));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "Space: include or leave out   Enter: save   Esc: back",
        Style::new().fg(DIM),
    ));
    let height = (rows_needed(&lines, 78) + 4).clamp(9, MAX_PROMPT_HEIGHT);
    ModalSpec {
        title: format!(" The areas of {name} "),
        lines,
        color: ACCENT,
        width: 78,
        height,
        buttons: vec![confirm_button("Save"), cancel_button()],
    }
}
