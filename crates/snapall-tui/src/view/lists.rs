// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Snapshots tab: the protected areas on the left, the chosen area's snapshots on the right.

use crate::view::*;

pub(crate) fn pane_block(title: String, focused: bool) -> Block<'static> {
    let color = if focused { ACCENT } else { DIM };
    Block::default()
        .borders(Borders::ALL)
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Plain
        })
        .border_style(Style::new().fg(color))
        .title(Span::styled(title, Style::new().fg(color).bold()))
}

pub(crate) fn render_configs(state: &State, frame: &mut Frame, area: Rect) {
    let focused = state.focus == Pane::Configs && state.modal.is_none();
    let block = pane_block(" Protected areas ".into(), focused);
    match &state.configs {
        Load::NotLoaded => {
            frame.render_widget(Paragraph::new("loading...").block(block), area);
        }
        Load::Failed(error) => {
            frame.render_widget(
                Paragraph::new(error.as_str())
                    .style(Style::new().fg(Color::Red))
                    .wrap(Wrap { trim: true })
                    .block(block),
                area,
            );
        }
        Load::Loaded(entries) if entries.is_empty() => {
            frame.render_widget(
                Paragraph::new("no protected areas yet")
                    .style(Style::new().fg(DIM))
                    .block(block),
                area,
            );
        }
        Load::Loaded(entries) => {
            let items: Vec<ListItem> = entries
                .iter()
                .map(|entry| {
                    let count = match &entry.snapshots {
                        Load::Loaded(list) => format!("{}", list.len()),
                        Load::Failed(_) => "!".to_owned(),
                        Load::NotLoaded => "-".to_owned(),
                    };
                    ListItem::new(Line::from(vec![
                        Span::raw(entry.name.to_string()),
                        Span::styled(format!("  ({count})"), Style::new().fg(DIM)),
                    ]))
                })
                .collect();
            let highlight = if focused {
                Style::new().fg(Color::Black).bg(ACCENT).bold()
            } else {
                Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            };
            let list = List::new(items)
                .block(block)
                .highlight_style(highlight)
                .highlight_symbol("> ");
            let mut list_state = ListState::default().with_selected(Some(state.config_cursor));
            frame.render_stateful_widget(list, area, &mut list_state);
        }
    }
}

pub(crate) fn kind_label(info: &SnapshotInfo) -> String {
    match info.kind {
        SnapshotKind::Single => "single".to_owned(),
        SnapshotKind::Pre => "pre".to_owned(),
        SnapshotKind::Post { pre } => format!("post<-{pre}"),
    }
}

pub(crate) fn render_snapshots(state: &State, frame: &mut Frame, area: Rect) {
    let focused = state.focus == Pane::Snapshots && state.modal.is_none();
    let Some(entry) = state.selected_config() else {
        frame.render_widget(pane_block(" Snapshots ".into(), focused), area);
        return;
    };
    let title = match &entry.snapshots {
        Load::Loaded(list) => format!(" Snapshots of {} ({}) ", entry.name, list.len()),
        _ => format!(" Snapshots of {} ", entry.name),
    };
    let block = pane_block(title, focused);

    match &entry.snapshots {
        Load::NotLoaded => {
            frame.render_widget(Paragraph::new("loading...").block(block), area);
        }
        Load::Failed(error) => {
            frame.render_widget(
                Paragraph::new(error.as_str())
                    .style(Style::new().fg(Color::Red))
                    .wrap(Wrap { trim: true })
                    .block(block),
                area,
            );
        }
        Load::Loaded(list) if list.is_empty() => {
            frame.render_widget(
                Paragraph::new("no snapshots yet. Press c to create one.")
                    .style(Style::new().fg(DIM))
                    .block(block),
                area,
            );
        }
        Load::Loaded(_) => {
            let rows: Vec<Row> = state
                .displayed_snapshots()
                .into_iter()
                .map(|info| {
                    let mark = if state.marked.contains(&info.id) {
                        "*"
                    } else {
                        " "
                    };
                    let style = if state.marked.contains(&info.id) {
                        Style::new().fg(Color::Yellow)
                    } else {
                        Style::new()
                    };
                    Row::new(vec![
                        mark.to_owned(),
                        info.id.to_string(),
                        kind_label(info),
                        info.date.clone().unwrap_or_default(),
                        info.cleanup.clone().unwrap_or_default(),
                        info.description.clone(),
                    ])
                    .style(style)
                })
                .collect();
            let header = Row::new(["", "#", "Type", "Date", "Cleanup", "Description"])
                .style(Style::new().fg(DIM).add_modifier(Modifier::BOLD));
            let highlight = if focused {
                Style::new().fg(Color::Black).bg(ACCENT).bold()
            } else {
                Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            };
            let table = Table::new(
                rows,
                [
                    Constraint::Length(1),
                    Constraint::Length(6),
                    Constraint::Length(9),
                    Constraint::Length(19),
                    Constraint::Length(8),
                    Constraint::Min(10),
                ],
            )
            .header(header)
            .block(block)
            .row_highlight_style(highlight)
            .column_spacing(1);
            let mut table_state = TableState::default().with_selected(Some(state.snapshot_cursor));
            frame.render_stateful_widget(table, area, &mut table_state);
        }
    }
}
