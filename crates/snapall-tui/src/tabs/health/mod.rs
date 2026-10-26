// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Health tab: is protection working, how much room does it take, will old snapshots be cleaned
//! up, and what to do about anything that is not.
//!
//! What it shows is decided in [`rows`], a pure function of the report, so it is tested without a
//! terminal. The rest is the cursor, the keys, and the drawing of those rows with the explanation of
//! the one under the cursor beneath them.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use snapall_core::explain::{cleanup_from, cleanup_text, room_counting_text, Cleanup};
use snapall_core::health::format_bytes;
use snapall_core::services::{self, Job};
use snapall_core::QuotaMode;
use snapall_core::{AreaFacts, Facts, Health, Level, Remedy, Topic};

use crate::model::{Effect, Key, Modal, Notice, State};
use crate::theme::{mark_of, of_level, ACCENT, DIM};

mod rows;
pub use rows::*;

// ---- what the tab holds -------------------------------------------------------------------------

/// The report the tab is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthData {
    pub facts: Facts,
    pub report: Health,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HealthTab {
    pub data: Option<HealthData>,
    /// Why the report could not be made, if it could not.
    pub error: Option<String>,
    /// Which row is chosen.
    pub cursor: usize,
}

impl HealthTab {
    pub fn rows(&self) -> Vec<Row> {
        self.data.as_ref().map(rows).unwrap_or_default()
    }

    /// Go to the first row about `topic`: where a click on the status bar lands.
    pub fn select_topic(&mut self, topic: Topic) {
        let rows = self.rows();
        self.cursor = rows
            .iter()
            .position(|row| row.topic == topic)
            .or_else(|| rows.iter().position(|row| row.section == Section::Space))
            .unwrap_or(0);
    }

    /// Keep the cursor on a row after the report changed under it.
    pub fn keep_cursor_in_range(&mut self) {
        let last = self.rows().len().saturating_sub(1);
        self.cursor = self.cursor.min(last);
    }
}

// ---- keys ---------------------------------------------------------------------------------------

/// The keys of this tab. `None` means the key is not one of its own, and the window handles it.
pub fn on_key(state: &mut State, key: Key) -> Option<Vec<Effect>> {
    let count = state.health.rows().len();
    let last = count.saturating_sub(1);
    match key {
        Key::Up | Key::Char('k') => state.health.cursor = state.health.cursor.saturating_sub(1),
        Key::Down | Key::Char('j') => state.health.cursor = (state.health.cursor + 1).min(last),
        Key::Home => state.health.cursor = 0,
        Key::End | Key::Char('G') => state.health.cursor = last,
        Key::Enter => return Some(act_on_chosen(state)),
        _ => return None,
    }
    Some(Vec::new())
}

/// Enter on a row: do what it offers, or open the controls it leads to.
fn act_on_chosen(state: &mut State) -> Vec<Effect> {
    let Some(row) = state.health.rows().into_iter().nth(state.health.cursor) else {
        return Vec::new();
    };
    match row.action {
        RowAction::Nothing => Vec::new(),
        RowAction::Fix(Remedy::SwitchOnCleanup) if !state.context.administrator => {
            state.notice = Some(Notice::Info(
                "Switching cleanup on needs administrator rights: start snapall with sudo.".into(),
            ));
            Vec::new()
        }
        RowAction::Fix(Remedy::SwitchOnCleanup) => {
            state.notice = Some(Notice::Info("switching automatic cleanup on".into()));
            vec![Effect::SwitchOnCleanup]
        }
        RowAction::Fix(Remedy::CountRoom) | RowAction::CountRoom(true) => {
            state.modal = Some(Modal::Quotas { turn_on: true });
            Vec::new()
        }
        RowAction::CountRoom(false) => {
            state.modal = Some(Modal::Quotas { turn_on: false });
            Vec::new()
        }
        RowAction::EditLimits(config) => crate::model::open_limits(state, config),
        RowAction::ControlJob(job) => {
            state.modal = Some(Modal::Job {
                unit: job.unit.to_owned(),
            });
            Vec::new()
        }
    }
}

// ---- drawing ------------------------------------------------------------------------------------

/// One line of the list: the verdict, a gap, a section's name, or a row.
enum Item {
    Verdict,
    Gap,
    Header(Section),
    Row(usize),
}

fn items(rows: &[Row]) -> Vec<Item> {
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
fn split(area: Rect) -> (Rect, Rect) {
    let detail = 6.min(area.height / 2);
    let [list, detail] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(detail)]).areas(area);
    (list, detail)
}

/// The first line shown, so that the chosen row is on screen.
fn offset(items: &[Item], cursor: usize, height: usize) -> usize {
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

fn verdict_line(report: &Health) -> Line<'static> {
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

fn row_line(row: &Row, chosen: bool) -> Line<'static> {
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

fn render_detail(state: &State, frame: &mut Frame, area: Rect, rows: &[Row]) {
    let chosen = rows.get(state.health.cursor);
    let hint = chosen.and_then(|row| match row.action {
        RowAction::Nothing => None,
        RowAction::Fix(Remedy::SwitchOnCleanup) => Some("Enter: switch automatic cleanup on"),
        RowAction::Fix(Remedy::CountRoom) | RowAction::CountRoom(true) => {
            Some("Enter: switch room counting on (it explains first)")
        }
        RowAction::CountRoom(false) => Some("Enter: switch room counting off (it explains first)"),
        RowAction::ControlJob(_) => Some("Enter: the commands that check on it and control it"),
        RowAction::EditLimits(_) => Some("Enter: change how many it keeps, and when the oldest go"),
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
