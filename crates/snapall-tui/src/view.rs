// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rendering: `render(state, frame)` draws, and does nothing else.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table,
    TableState, Wrap,
};
use ratatui::Frame;
use snapall_core::{SnapshotInfo, SnapshotKind};

use crate::model::{Load, Modal, Notice, Pane, State, Target};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

pub fn render(state: &State, frame: &mut Frame) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(frame.area());

    render_header(state, frame, header);
    let [configs, snapshots] =
        Layout::horizontal([Constraint::Length(28), Constraint::Min(20)]).areas(body);
    render_configs(state, frame, configs);
    render_snapshots(state, frame, snapshots);
    render_footer(state, frame, footer);

    if let Some(modal) = &state.modal {
        render_modal(modal, state, frame);
    }
}

fn render_header(state: &State, frame: &mut Frame, area: Rect) {
    let mut spans = vec![
        Span::styled(" snapall ", Style::new().fg(Color::Black).bg(ACCENT).bold()),
        Span::styled("  snapper snapshots", Style::new().fg(DIM)),
    ];
    if state.pending > 0 {
        // Work is in flight on the worker thread: say so, and keep the screen moving.
        const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
        let word = if state.quit { "finishing" } else { "working" };
        spans.push(Span::styled(
            format!("   {} {word}", FRAMES[state.spinner % FRAMES.len()]),
            Style::new().fg(Color::Yellow).bold(),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn pane_block(title: String, focused: bool) -> Block<'static> {
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

fn render_configs(state: &State, frame: &mut Frame, area: Rect) {
    let focused = state.focus == Pane::Configs && state.modal.is_none();
    let block = pane_block(" Configs ".into(), focused);
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
                Paragraph::new("no snapper configs")
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

fn kind_label(info: &SnapshotInfo) -> String {
    match info.kind {
        SnapshotKind::Single => "single".to_owned(),
        SnapshotKind::Pre => "pre".to_owned(),
        SnapshotKind::Post { pre } => format!("post<-{pre}"),
    }
}

fn render_snapshots(state: &State, frame: &mut Frame, area: Rect) {
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

fn render_footer(state: &State, frame: &mut Frame, area: Rect) {
    let [notice_area, keys_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    let notice = match &state.notice {
        Some(Notice::Info(text)) => Span::styled(format!(" {text}"), Style::new().fg(Color::Green)),
        Some(Notice::Error(text)) => Span::styled(format!(" {text}"), Style::new().fg(Color::Red)),
        None => Span::raw(""),
    };
    frame.render_widget(Paragraph::new(Line::from(notice)), notice_area);

    // Short enough to fit an 80-column terminal (checked by a rendering test).
    let hints = [
        ("j/k", "move"),
        ("h/l", "pane"),
        ("space", "mark"),
        ("c", "create"),
        ("a", "all"),
        ("d", "delete"),
        ("r", "reload"),
        ("?", "help"),
        ("q", "quit"),
    ];
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in hints {
        spans.push(Span::styled(key, Style::new().fg(ACCENT).bold()));
        spans.push(Span::styled(format!(" {label}  "), Style::new().fg(DIM)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), keys_area);
}

/// A rectangle `width` x `height`, centred in `area` and never larger than it.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn render_modal(modal: &Modal, state: &State, frame: &mut Frame) {
    match modal {
        Modal::Describe {
            target,
            input,
            error,
        } => {
            let title = match target {
                Target::One(config) => format!(" New snapshot of {config} "),
                Target::All => {
                    let names: Vec<String> =
                        state.entries().iter().map(|e| e.name.to_string()).collect();
                    format!(" New snapshot of ALL configs ({}) ", names.join(", "))
                }
            };
            let mut lines = vec![
                Line::from("Description (Enter alone uses \"snapall\"):"),
                Line::from(vec![
                    Span::styled(input.clone(), Style::new().bold()),
                    Span::styled(
                        "_",
                        Style::new().fg(ACCENT).add_modifier(Modifier::SLOW_BLINK),
                    ),
                ]),
                Line::from(""),
            ];
            if let Some(error) = error {
                lines.push(Line::styled(error.clone(), Style::new().fg(Color::Red)));
            }
            lines.push(Line::styled(
                "Enter create   Esc cancel",
                Style::new().fg(DIM),
            ));
            draw_modal(frame, title, lines, ACCENT, 64, 8);
        }
        Modal::ConfirmDelete { config, ids } => {
            let list: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
            let lines = vec![
                Line::from(format!("Delete {} snapshot(s) of {config}?", ids.len())),
                Line::styled(list.join(", "), Style::new().bold()),
                Line::from(""),
                Line::styled("This cannot be undone.", Style::new().fg(Color::Yellow)),
                Line::styled("y / Enter delete   n / Esc cancel", Style::new().fg(DIM)),
            ];
            draw_modal(frame, " Delete ".into(), lines, Color::Red, 56, 9);
        }
        Modal::Help => {
            let rows = [
                ("up/down  k/j", "move in the focused pane"),
                ("left/right  h/l  Tab", "switch pane"),
                ("Home/End  g/G", "first / last"),
                ("space", "mark a snapshot, then move on"),
                ("c", "create a snapshot of the selected config"),
                ("a", "create a snapshot of every config"),
                (
                    "d",
                    "delete the marked snapshots (or the one under the cursor)",
                ),
                ("r", "reload from snapper"),
                ("q  Ctrl-C", "quit"),
            ];
            let mut lines: Vec<Line> = rows
                .iter()
                .map(|(key, what)| {
                    Line::from(vec![
                        Span::styled(format!("{key:<22}"), Style::new().fg(ACCENT).bold()),
                        Span::raw(*what),
                    ])
                })
                .collect();
            lines.push(Line::from(""));
            lines.push(Line::styled("any key closes this", Style::new().fg(DIM)));
            draw_modal(frame, " Keys ".into(), lines, ACCENT, 72, 14);
        }
    }
}

fn draw_modal(
    frame: &mut Frame,
    title: String,
    lines: Vec<Line<'static>>,
    color: Color,
    width: u16,
    height: u16,
) {
    let area = centered(frame.area(), width, height);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(Span::styled(title, Style::new().fg(color).bold()))
        .title_alignment(Alignment::Left);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}
