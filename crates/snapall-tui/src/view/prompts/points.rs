// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The restore points screen.

use crate::view::*;

/// The restore points: the list with the cursor on one, and what putting that one back would do.
pub(crate) fn points_spec(index: usize, state: &State) -> ModalSpec {
    let simple = |lines: Vec<Line<'static>>, height: u16| ModalSpec {
        title: " Restore points ".into(),
        lines,
        color: ACCENT,
        width: 78,
        height,
        buttons: vec![close_button()],
    };
    let Some(points) = &state.points else {
        return simple(
            vec![Line::styled(
                "Looking for restore points...",
                Style::new().fg(DIM),
            )],
            8,
        );
    };
    if points.is_empty() {
        return simple(
            vec![
                Line::from("There are no restore points yet."),
                Line::from(""),
                Line::from("Taking a snapshot of every area at once (press a, or run sudo snapall) makes one: the snapshots one action takes together, so that everything can be put back to that moment in one step."),
            ],
            11,
        );
    }
    let index = index.min(points.len() - 1);
    let mut lines = vec![
        Line::from("Choose a restore point (Up and Down):"),
        Line::from(""),
    ];
    let first = index.saturating_sub(5);
    for (at, point) in points.iter().enumerate().skip(first).take(6) {
        let marker = if at == index { "> " } else { "  " };
        let note = if point.tagged {
            ""
        } else {
            " (older: matched by name and time)"
        };
        let row = format!(
            "{marker}{}  {}  {}{note}",
            point.point,
            point.date.as_deref().unwrap_or(""),
            point.description
        );
        let row: String = row.chars().take(74).collect();
        lines.push(if at == index {
            Line::styled(row, Style::new().fg(Color::Yellow).bold())
        } else {
            Line::styled(row, Style::new().fg(DIM))
        });
    }
    lines.push(Line::from(""));
    let preview = state
        .point_preview
        .as_ref()
        .filter(|p| p.point.point == points[index].point);
    match preview {
        None => lines.push(Line::styled(
            "Looking at what it would change...",
            Style::new().fg(DIM),
        )),
        Some(preview) => {
            lines.push(Line::from("Putting every area back would:"));
            let width = preview
                .views
                .iter()
                .map(|v| v.area.as_str().len())
                .max()
                .unwrap_or(0);
            for view in &preview.views {
                let what = match &view.view {
                    snapall_core::MemberView::Changes(p) if p.changes.is_empty() => {
                        "nothing: it already matches".to_owned()
                    }
                    snapall_core::MemberView::Changes(p) => {
                        let (changed, added, removed) = restore_counts(&p.changes);
                        format!("change {} path(s): {changed} reverted, {added} removed, {removed} brought back", p.changes.len())
                    }
                    snapall_core::MemberView::SystemRoot => {
                        format!(
                            "left alone: the running system is not put back (its snapshot here is #{})",
                            view.id
                        )
                    }
                    snapall_core::MemberView::Failed(why) => {
                        format!("could not be looked at: {why}")
                    }
                };
                lines.push(Line::from(format!("  {:<width$}  {what}", view.area)));
            }
        }
    }
    let mut buttons = Vec::new();
    if let Some(preview) = preview.filter(|p| p.areas_to_change() > 0) {
        buttons.push(confirm_button(&format!(
            "Restore all {}",
            preview.areas_to_change()
        )));
    }
    buttons.push(close_button());
    let height = u16::try_from(lines.len() + 3)
        .unwrap_or(20)
        .clamp(10, MAX_PROMPT_HEIGHT);
    ModalSpec {
        title: format!(" Restore points  ({} of {}) ", index + 1, points.len()),
        lines,
        color: Color::Yellow,
        width: 78,
        height,
        buttons,
    }
}
