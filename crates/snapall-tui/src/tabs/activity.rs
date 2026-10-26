// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Activity tab: what snapall did, newest first, and how each went. It shows the journal, so it
//! reaches back past this window to earlier sessions and shows what the commands did too; a failure
//! stays visible with its reason long after the one-line message has gone, and a change whose program
//! was closed or killed is shown as stopped instead of vanishing.
//!
//! Where nothing is remembered (a machine with no journal) it shows what this window did, so the tab
//! is never empty of what just happened.

use std::collections::HashSet;
use std::time::Duration;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use snapall_core::{JournalEntry, JournalStatus, Source};

use crate::model::{Effect, Key, State};
use crate::theme::DIM;

/// How many entries are kept: enough to scroll back through a long history, not enough to grow.
const KEPT: usize = 200;

/// One thing snapall did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// Which change it is in the journal, so that it is shown once however often it is read.
    pub id: Option<String>,
    /// When it finished (or started, if it has not), as the machine's clock wrote it.
    pub at: Option<String>,
    /// What it was, in a sentence: `Took a snapshot of home, root`.
    pub what: String,
    pub status: JournalStatus,
    /// What came of it, or why it failed.
    pub detail: String,
    pub took: Option<Duration>,
    pub source: Option<Source>,
}

impl Activity {
    pub fn from_entry(entry: &JournalEntry) -> Activity {
        Activity {
            id: Some(entry.id.clone()),
            at: entry
                .finished_at
                .clone()
                .or_else(|| entry.started_at.clone()),
            what: entry.what.clone(),
            status: entry.status,
            detail: entry.detail.clone(),
            took: entry.took_ms.map(Duration::from_millis),
            source: entry.source,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActivityTab {
    /// Newest first.
    pub entries: Vec<Activity>,
    pub cursor: usize,
}

impl ActivityTab {
    /// This window did something: it is shown at once, before the journal has been read again.
    pub fn record(&mut self, activity: Activity) {
        self.entries.insert(0, activity);
        self.entries.truncate(KEPT);
        self.cursor = 0; // the newest is what a person looks for
    }

    /// The journal was read. It is the record; what this window did that the journal does not have (no
    /// journal on this machine) stays, newest.
    pub fn merge_history(&mut self, history: &[JournalEntry]) {
        let known: HashSet<&str> = history.iter().map(|entry| entry.id.as_str()).collect();
        let mut merged: Vec<Activity> = std::mem::take(&mut self.entries)
            .into_iter()
            .filter(|own| own.id.as_deref().is_none_or(|id| !known.contains(id)))
            .collect();
        merged.extend(history.iter().map(Activity::from_entry));
        merged.truncate(KEPT);
        self.entries = merged;
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }
}

/// The keys of this tab; `None` is a key the window handles.
pub fn on_key(state: &mut State, key: Key) -> Option<Vec<Effect>> {
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

/// `2026-10-25 18:15:02` as `18:15:02`, or as `10-25 18:15` when the list spans more than one day.
fn when(at: Option<&str>, several_days: bool) -> String {
    let Some((date, time)) = at.and_then(|text| text.split_once(' ')) else {
        return if several_days {
            "-- --:--".to_owned()
        } else {
            "--:--:--".to_owned()
        };
    };
    if several_days {
        let day = date.get(5..).unwrap_or(date);
        format!("{day} {}", time.get(..5).unwrap_or(time))
    } else {
        time.to_owned()
    }
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

/// Whether the times need their day: when any entry is not from today, or, if today is not known, when
/// the entries are from more than one day. A list of yesterday's changes must not look like today's.
fn needs_dates(entries: &[Activity], today: Option<&str>) -> bool {
    let days: HashSet<&str> = entries
        .iter()
        .filter_map(|entry| entry.at.as_deref()?.split_once(' ').map(|(date, _)| date))
        .collect();
    match today {
        Some(today) => days.iter().any(|day| *day != today),
        None => days.len() > 1,
    }
}

/// The machine's date, from the report of how protection stands, if that has arrived.
fn today(state: &State) -> Option<&str> {
    let now = state.health.data.as_ref()?.facts.now.as_deref()?;
    now.split_once(' ').map(|(date, _)| date)
}

pub fn render(state: &State, frame: &mut Frame, area: Rect) {
    let (note, list, detail) = split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                " What snapall did, from this window and from commands, newest first.",
                Style::new().fg(DIM),
            ),
            Line::styled(
                " It is remembered between sessions, and a change that was cut short shows as STOPPED.",
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
    let several_days = needs_dates(&tab.entries, today(state));
    let first = first_shown(tab.cursor, usize::from(list.height));
    let lines: Vec<Line> = tab
        .entries
        .iter()
        .enumerate()
        .skip(first)
        .take(usize::from(list.height))
        .map(|(index, entry)| entry_line(entry, index == tab.cursor, several_days))
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
    render_detail(state, frame, detail);
}

fn entry_line(entry: &Activity, chosen: bool, several_days: bool) -> Line<'static> {
    let body = Style::new().add_modifier(if chosen {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    });
    let color = match entry.status {
        JournalStatus::Done => Color::Green,
        JournalStatus::Running => Color::Yellow,
        JournalStatus::Failed | JournalStatus::Interrupted => Color::Red,
    };
    let origin = match entry.source {
        Some(Source::Command) => " (command)",
        _ => "",
    };
    let mut spans = vec![
        Span::styled(if chosen { " > " } else { "   " }, body),
        Span::styled(when(entry.at.as_deref(), several_days), body.fg(DIM)),
        Span::styled(format!("  {:<7}", entry.status.word()), body.fg(color)),
        Span::styled(format!(" {}{origin}", entry.what), body),
    ];
    if let Some(duration) = entry.took {
        spans.push(Span::styled(
            format!("  ({})", took(duration)),
            body.fg(DIM),
        ));
    }
    Line::from(spans)
}

/// What the chosen entry says about itself, in words.
fn detail_text(entry: &Activity) -> String {
    match entry.status {
        JournalStatus::Done if entry.detail.is_empty() => "It went through.".to_owned(),
        JournalStatus::Done | JournalStatus::Interrupted => entry.detail.clone(),
        JournalStatus::Failed => format!("It failed: {}", entry.detail),
        JournalStatus::Running => "It is being done right now.".to_owned(),
    }
}

fn render_detail(state: &State, frame: &mut Frame, area: Rect) {
    let text = state
        .activity
        .entries
        .get(state.activity.cursor)
        .map(detail_text)
        .unwrap_or_default();
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
