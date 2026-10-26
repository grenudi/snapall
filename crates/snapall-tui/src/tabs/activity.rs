// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Activity tab: what snapall did in this window and how it went, newest first. A failure stays
//! visible here after the one-line message that announced it has gone.
//!
//! It is the record of this window, not a log file: for a permanent one, start snapall with
//! `--log-file`, and the tab says so.

use std::time::Duration;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::model::{Key, State};
use crate::theme::DIM;

/// How many entries are kept: enough to scroll back through a long session, not enough to grow.
const KEPT: usize = 200;

/// One thing snapall did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// When it finished, as the machine's clock wrote it, if it could be read.
    pub at: Option<String>,
    /// What it was, in a sentence: `Took a snapshot of Home, System`.
    pub what: String,
    /// How it ended: what came of it, or why it failed.
    pub outcome: Result<String, String>,
    pub took: Duration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityTab {
    /// Newest first.
    pub entries: Vec<Activity>,
    pub cursor: usize,
}

impl ActivityTab {
    pub fn record(&mut self, activity: Activity) {
        self.entries.insert(0, activity);
        self.entries.truncate(KEPT);
        self.cursor = 0; // the newest is what a person looks for
    }
}

/// The keys of this tab; `None` is a key the window handles.
pub fn on_key(state: &mut State, key: Key) -> Option<Vec<crate::model::Effect>> {
    let last = state.activity.entries.len().saturating_sub(1);
    let tab = &mut state.activity;
    match key {
        Key::Up | Key::Char('k') => tab.cursor = tab.cursor.saturating_sub(1),
        Key::Down | Key::Char('j') => tab.cursor = (tab.cursor + 1).min(last),
        Key::Home | Key::Char('g') => tab.cursor = 0,
        Key::End | Key::Char('G') => tab.cursor = last,
        _ => return None,
    }
    Some(Vec::new())
}

/// The time of day out of `2026-10-25 18:15:02`.
fn time_of_day(at: &Option<String>) -> String {
    at.as_deref()
        .and_then(|text| text.split_once(' ').map(|(_, time)| time.to_owned()))
        .unwrap_or_else(|| "--:--:--".to_owned())
}

fn took(duration: Duration) -> String {
    if duration.as_millis() < 1000 {
        format!("{} ms", duration.as_millis())
    } else {
        format!("{:.1} s", duration.as_secs_f64())
    }
}

fn split(area: Rect) -> (Rect, Rect, Rect) {
    let detail = 5.min(area.height / 3);
    let [note, list, detail] = Layout::vertical([
        Constraint::Length(2.min(area.height)),
        Constraint::Min(1),
        Constraint::Length(detail),
    ])
    .areas(area);
    (note, list, detail)
}

fn first_shown(cursor: usize, height: usize) -> usize {
    (cursor + 1).saturating_sub(height)
}

pub fn render(state: &State, frame: &mut Frame, area: Rect) {
    let (note, list, detail) = split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                " What snapall did in this window, newest first.",
                Style::new().fg(DIM),
            ),
            Line::styled(
                " For a record that outlasts it, start snapall with --log-file FILE.",
                Style::new().fg(DIM),
            ),
        ]),
        note,
    );
    let tab = &state.activity;
    if tab.entries.is_empty() {
        frame.render_widget(
            Paragraph::new(" Nothing yet: taking, deleting or restoring a snapshot shows up here.")
                .style(Style::new().fg(DIM)),
            list,
        );
        return;
    }
    let first = first_shown(tab.cursor, usize::from(list.height));
    let lines: Vec<Line> = tab
        .entries
        .iter()
        .enumerate()
        .skip(first)
        .take(usize::from(list.height))
        .map(|(index, entry)| entry_line(entry, index == tab.cursor))
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
    render_detail(state, frame, detail);
}

fn entry_line(entry: &Activity, chosen: bool) -> Line<'static> {
    let body = Style::new().add_modifier(if chosen {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    });
    let (mark, color) = match entry.outcome {
        Ok(_) => ("done  ", Color::Green),
        Err(_) => ("FAILED", Color::Red),
    };
    Line::from(vec![
        Span::styled(if chosen { " > " } else { "   " }, body),
        Span::styled(time_of_day(&entry.at), body.fg(DIM)),
        Span::styled(format!("  {mark}"), body.fg(color)),
        Span::styled(format!("  {}", entry.what), body),
        Span::styled(format!("  ({})", took(entry.took)), body.fg(DIM)),
    ])
}

fn render_detail(state: &State, frame: &mut Frame, area: Rect) {
    let text = match state.activity.entries.get(state.activity.cursor) {
        Some(entry) => match &entry.outcome {
            Ok(done) if done.is_empty() => "It went through.".to_owned(),
            Ok(done) => done.clone(),
            Err(why) => format!("It failed: {why}"),
        },
        None => String::new(),
    };
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(DIM))
        .title(Span::styled(" How it went ", Style::new().fg(DIM)));
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: true }).block(block),
        area,
    );
}

/// Which entry a click at screen row `y` is on.
pub fn row_at(state: &State, area: Rect, y: u16) -> Option<usize> {
    let (_, list, _) = split(area);
    if y < list.y || y >= list.y.saturating_add(list.height) {
        return None;
    }
    let first = first_shown(state.activity.cursor, usize::from(list.height));
    let index = first + usize::from(y - list.y);
    (index < state.activity.entries.len()).then_some(index)
}
