// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The file history screen: one file's versions, newest first.

use crate::view::*;

pub(crate) fn state_style(state: FileState) -> Style {
    match state {
        FileState::Changed => Style::new().fg(Color::Yellow),
        FileState::DeletedSince => Style::new().fg(Color::Red),
        FileState::Same | FileState::AbsentThen => Style::new().fg(DIM),
    }
}

/// The history of one file: a row per snapshot, newest first, saying how the file stood in it.
pub(crate) fn render_file(state: &State, frame: &mut Frame, area: Rect) {
    let Some(view) = &state.file else {
        return;
    };
    let name = view
        .target
        .path
        .rsplit('/')
        .next()
        .unwrap_or(view.target.path.as_str());
    let title = format!(" {name} in {} ", view.target.config);
    let block = pane_block(title, state.modal.is_none());
    match &view.versions {
        Load::NotLoaded => frame.render_widget(Paragraph::new("loading...").block(block), area),
        Load::Failed(error) => frame.render_widget(
            Paragraph::new(error.as_str())
                .style(Style::new().fg(Color::Red))
                .wrap(Wrap { trim: true })
                .block(block),
            area,
        ),
        Load::Loaded(list) if list.is_empty() => frame.render_widget(
            Paragraph::new("no snapshots of this area yet. Press q to quit.")
                .style(Style::new().fg(DIM))
                .block(block),
            area,
        ),
        Load::Loaded(_) => {
            let rows: Vec<Row> = state
                .displayed_versions()
                .into_iter()
                .map(|version| {
                    Row::new(vec![
                        version.snapshot.id.to_string(),
                        version.snapshot.date.clone().unwrap_or_default(),
                        version.state.label().to_owned(),
                        version.snapshot.description.clone(),
                    ])
                    .style(state_style(version.state))
                })
                .collect();
            let header = Row::new(["#", "Date", "This file", "Snapshot"])
                .style(Style::new().fg(DIM).add_modifier(Modifier::BOLD));
            let highlight = if state.modal.is_none() {
                Style::new().fg(Color::Black).bg(ACCENT).bold()
            } else {
                Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            };
            let table = Table::new(
                rows,
                [
                    Constraint::Length(6),
                    Constraint::Length(19),
                    Constraint::Length(17),
                    Constraint::Min(10),
                ],
            )
            .header(header)
            .block(block)
            .row_highlight_style(highlight)
            .column_spacing(1);
            let mut table_state = TableState::default().with_selected(Some(view.cursor));
            frame.render_stateful_widget(table, area, &mut table_state);
        }
    }
}
