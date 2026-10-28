// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawing the Health tab: the rows, the verdict above them and the detail of the chosen one.

use super::*;
use crate::model::State;
use crate::theme::{mark_of, of_level, ACCENT, DIM};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use snapall_core::{Health, Remedy};

/// One line of the list: the verdict, a gap, a section's name, or a row.
pub(super) enum Item {
    Verdict,
    Gap,
    Header(Section),
    Row(usize),
}

pub(super) fn items(rows: &[Row]) -> Vec<Item> {
    let mut items = vec![Item::Verdict, Item::Gap];
    let mut section = None;
    for (index, row) in rows.iter().enumerate() {
        if section != Some(row.section) {
            if section.is_some() {
                items.push(Item::Gap);
            }
            items.push(Item::Header(row.section));
            section = Some(row.section);
        }
        items.push(Item::Row(index));
    }
    items
}

/// The list on top, the explanation of the chosen row beneath it.
pub(super) fn split(area: Rect) -> (Rect, Rect) {
    let detail = 6.min(area.height / 2);
    let [list, detail] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(detail)]).areas(area);
    (list, detail)
}

/// The first line shown, so that the chosen row is on screen.
pub(super) fn offset(items: &[Item], cursor: usize, height: usize) -> usize {
    let at = items
        .iter()
        .position(|item| matches!(item, Item::Row(index) if *index == cursor))
        .unwrap_or(0);
    (at + 1).saturating_sub(height)
}

pub fn render(state: &State, frame: &mut Frame, area: Rect) {
    let Some(data) = &state.health.data else {
        let text = state
            .health
            .error
            .clone()
            .unwrap_or_else(|| "Looking at how protection stands...".to_owned());
        frame.render_widget(Paragraph::new(text).style(Style::new().fg(DIM)), area);
        return;
    };
    let rows = rows(data);
    let items = items(&rows);
    let (list, detail) = split(area);
    let first = offset(&items, state.health.cursor, usize::from(list.height));
    let lines: Vec<Line> = items
        .iter()
        .skip(first)
        .take(usize::from(list.height))
        .map(|item| match item {
            Item::Verdict => verdict_line(&data.report),
            Item::Gap => Line::from(""),
            Item::Header(section) => Line::styled(
                format!(" {}", section.title()),
                Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Item::Row(index) => row_line(&rows[*index], *index == state.health.cursor),
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
    render_detail(state, frame, detail, &rows);
}

pub(super) fn verdict_line(report: &Health) -> Line<'static> {
    Line::from(vec![
        Span::styled(" Health: ", Style::new().fg(DIM)),
        Span::styled(
            report.verdict.text.clone(),
            Style::new()
                .fg(of_level(report.verdict.level))
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

pub(super) fn row_line(row: &Row, chosen: bool) -> Line<'static> {
    let body = Style::new().add_modifier(if chosen {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    });
    Line::from(vec![
        Span::styled(if chosen { " > " } else { "   " }, body),
        Span::styled(mark_of(row.level), body.fg(of_level(row.level))),
        Span::styled(format!(" {}", row.label), body),
    ])
}

pub(super) fn render_detail(state: &State, frame: &mut Frame, area: Rect, rows: &[Row]) {
    let chosen = rows.get(state.health.cursor);
    let hint = chosen.and_then(|row| match row.action {
        RowAction::Nothing => None,
        RowAction::Fix(Remedy::SwitchOnCleanup) => Some("Enter: switch automatic cleanup on"),
        RowAction::Fix(Remedy::CountRoom) | RowAction::CountRoom(true) => {
            Some("Enter: switch room counting on (it explains first)")
        }
        RowAction::CountRoom(false) => Some("Enter: switch room counting off (it explains first)"),
        RowAction::ControlJob(_) => Some("Enter: the commands that check on it and control it"),
        RowAction::EditLimits(_) => {
            Some("Enter: change what it keeps   U: stop protecting it (deletes its snapshots)")
        }
    });
    // what Enter does is the one line that must never be cut off by a long explanation: it has its own
    let (text_area, hint_area) = if hint.is_some() && area.height > 2 {
        let text = Rect {
            height: area.height - 1,
            ..area
        };
        let hint = Rect {
            y: area.y + area.height - 1,
            height: 1,
            ..area
        };
        (text, Some(hint))
    } else {
        (area, None)
    };
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(DIM))
        .title(Span::styled(" What this means ", Style::new().fg(DIM)));
    let text = chosen.map(|row| row.detail.clone()).unwrap_or_default();
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: true }).block(block),
        text_area,
    );
    if let (Some(hint), Some(rect)) = (hint, hint_area) {
        frame.render_widget(
            Paragraph::new(Line::styled(
                format!(" {hint}"),
                Style::new()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            rect,
        );
    }
}

/// Which row a click at screen row `y` is on, using the same layout the drawing used.
pub fn row_at(state: &State, area: Rect, y: u16) -> Option<usize> {
    let data = state.health.data.as_ref()?;
    let rows = rows(data);
    let items = items(&rows);
    let (list, _) = split(area);
    if y < list.y || y >= list.y.saturating_add(list.height) {
        return None;
    }
    let first = offset(&items, state.health.cursor, usize::from(list.height));
    match items.get(first + usize::from(y - list.y))? {
        Item::Row(index) => Some(*index),
        _ => None,
    }
}
