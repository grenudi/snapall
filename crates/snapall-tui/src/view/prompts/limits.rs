// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The editor of how many snapshots an area keeps, with what the settings would do shown beneath them.

use crate::model::{now_seconds, LimitsEditor};
use crate::view::*;
use snapall_core::{limits_words, preview_limits};

pub(crate) fn limits_spec(editor: &LimitsEditor, state: &State) -> ModalSpec {
    let area = snapall_core::coverage::describe(
        &state
            .health
            .data
            .as_ref()
            .and_then(|data| data.facts.areas.iter().find(|a| a.config == editor.config))
            .map_or_else(String::new, |area| area.subvolume.clone()),
    )
    .title;
    let field = |index: usize, label: &str, value: String, note: &str| {
        let chosen = editor.field == index;
        let style = if chosen {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        Line::from(vec![
            Span::styled(if chosen { " > " } else { "   " }, style),
            Span::styled(format!("{label:<34}"), style),
            Span::styled(format!("[ {value} ]"), style.bold()),
            Span::styled(format!("  {note}"), Style::new().fg(DIM)),
        ])
    };
    let mut lines = vec![
        Line::from("Cleanup removes the oldest snapshots of an area once it has more than it is set to keep. Snapshots you take yourself are among those it looks after."),
        Line::from(""),
        field(0, "Keep the newest", editor.limits.keep.to_string(), "snapshots"),
        field(
            1,
            "Never remove one younger than",
            (editor.limits.min_age_seconds / 60).to_string(),
            "minutes",
        ),
        field(2, "Cleanup of this area", if editor.limits.cleanup { "on".into() } else { "off".into() }, ""),
        Line::from(""),
    ];
    match &editor.snapshots {
        None => lines.push(Line::styled(
            "Looking at its snapshots...",
            Style::new().fg(DIM),
        )),
        Some(snapshots) => {
            let preview = preview_limits(snapshots, &editor.limits, now_seconds(state));
            let colour = if preview.would_remove > 0 {
                Color::Yellow
            } else {
                Color::Green
            };
            lines.push(Line::styled(
                limits_words(&preview, &editor.limits),
                Style::new().fg(colour),
            ));
        }
    }
    if let Some(problem) = &editor.error {
        lines.push(Line::styled(problem.clone(), Style::new().fg(Color::Red)));
    }
    if editor.armed {
        lines.push(Line::styled(
            "Press Enter again to apply this. Those snapshots will be removed at the next cleanup.",
            Style::new().fg(Color::Red).bold(),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "Up/Down: choose   Left/Right or - +: change   PgUp/PgDn: by ten   Enter: apply   Esc: cancel",
        Style::new().fg(DIM),
    ));
    let width = 80;
    let height = (rows_needed(&lines, width) + 4).clamp(12, 21);
    ModalSpec {
        title: format!(" What {area} keeps "),
        lines,
        color: ACCENT,
        width,
        height,
        buttons: vec![confirm_button("Apply"), cancel_button()],
    }
}
