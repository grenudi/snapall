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

/// The groups under the areas: a heading, up to three groups, and how many more there are; or, when there
/// are none, a line that says how to make one, so that the feature can be found by looking.
/// `text` cut to `width` columns, with an ellipsis where it was cut: a clipped word is read as a different one.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

fn group_lines(state: &State, width: usize) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled("Groups", Style::new().fg(ACCENT).bold()),
        Span::styled("  g: manage", Style::new().fg(DIM)),
    ])];
    let names: Vec<&snapall_core::GroupName> = state.groups.names().collect();
    if names.is_empty() {
        lines.push(Line::styled("none yet: press g", Style::new().fg(DIM)));
        return lines;
    }
    for name in names.iter().take(3) {
        let areas: Vec<String> = state
            .groups
            .get(name)
            .unwrap_or(&[])
            .iter()
            .map(ToString::to_string)
            .collect();
        let name = name.to_string();
        let room = width.saturating_sub(name.chars().count() + 2);
        lines.push(Line::from(vec![
            Span::styled(name, Style::new().bold()),
            Span::styled(
                format!("  {}", fit(&areas.join(", "), room)),
                Style::new().fg(DIM),
            ),
        ]));
    }
    if names.len() > 3 {
        lines.push(Line::styled(
            format!("+{} more", names.len() - 3),
            Style::new().fg(DIM),
        ));
    }
    lines
}

/// The left pane's inside, split: the areas, and beneath them (if there is room for every area and a gap
/// as well) the groups. Drawing and clicking both use this, so a click lands on what was drawn.
pub(crate) fn areas_split(state: &State, inner: Rect) -> (Rect, Option<Rect>) {
    let wanted =
        u16::try_from(group_lines(state, usize::from(inner.width)).len()).unwrap_or(u16::MAX);
    let areas = u16::try_from(state.entries().len().max(1)).unwrap_or(u16::MAX);
    if state.file.is_some() || inner.height < areas.saturating_add(wanted).saturating_add(1) {
        return (inner, None);
    }
    // right under the areas they are made of, after a blank line
    let list = Rect {
        height: areas,
        ..inner
    };
    let groups = Rect {
        y: inner.y + areas + 1,
        height: wanted,
        ..inner
    };
    (list, Some(groups))
}

pub(crate) fn render_areas(state: &State, frame: &mut Frame, area: Rect) {
    let focused = state.focus == Pane::Areas && state.modal.is_none();
    let block = pane_block(" Protected areas ".into(), focused);
    match &state.areas {
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
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let (list_area, groups_area) = areas_split(state, inner);
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
                .highlight_style(highlight)
                .highlight_symbol("> ");
            let mut list_state = ListState::default().with_selected(Some(state.area_cursor));
            frame.render_stateful_widget(list, list_area, &mut list_state);
            if let Some(groups) = groups_area {
                let hovered = state.hover == Some(Click::Button(Button::Groups));
                let style = if hovered {
                    Style::new().add_modifier(Modifier::REVERSED)
                } else {
                    Style::new()
                };
                frame.render_widget(
                    Paragraph::new(group_lines(state, usize::from(groups.width))).style(style),
                    groups,
                );
            }
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
    let Some(entry) = state.selected_area() else {
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
