// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawing the Activity tab: a line for each change, and the detail of the chosen one.

use super::*;
use crate::model::State;
use crate::theme::DIM;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use snapall_core::{JournalStatus, Source};
use std::collections::HashSet;
use std::time::Duration;

/// `2026-10-25 18:15:02` as `18:15:02`, or as `10-25 18:15` when the list spans more than one day.
pub(super) fn when(at: Option<&str>, several_days: bool) -> String {
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

pub(super) fn took(duration: Duration) -> String {
    if duration.as_millis() < 1000 {
        format!("{} ms", duration.as_millis())
    } else {
        format!("{:.1} s", duration.as_secs_f64())
    }
}

pub(super) fn split(area: Rect) -> (Rect, Rect, Rect) {
    let detail = 5.min(area.height / 3);
    let [note, list, detail] = Layout::vertical([
        Constraint::Length(2.min(area.height)),
        Constraint::Min(1),
        Constraint::Length(detail),
    ])
    .areas(area);
    (note, list, detail)
}

pub(super) fn first_shown(cursor: usize, height: usize) -> usize {
    (cursor + 1).saturating_sub(height)
}

/// Whether the times need their day: when any entry is not from today, or, if today is not known, when
/// the entries are from more than one day. A list of yesterday's changes must not look like today's.
pub(super) fn needs_dates(entries: &[Activity], today: Option<&str>) -> bool {
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
pub(super) fn today(state: &State) -> Option<&str> {
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

pub(super) fn entry_line(entry: &Activity, chosen: bool, several_days: bool) -> Line<'static> {
    let body = Style::new().add_modifier(if chosen {
        Modifier::REVERSED
    } else {
        Modifier::empty()
    });
    let color = match entry.status {
        JournalStatus::Done => Color::Green,
        JournalStatus::Running | JournalStatus::Cancelled => Color::Yellow,
        JournalStatus::Failed | JournalStatus::Interrupted => Color::Red,
    };
    let origin = match entry.source {
        Some(Source::Command) => " (command)",
        _ => "",
    };
    let mut spans = vec![
        Span::styled(if chosen { " > " } else { "   " }, body),
        Span::styled(when(entry.at.as_deref(), several_days), body.fg(DIM)),
        Span::styled(format!("  {:<9}", entry.status.word()), body.fg(color)),
        Span::styled(format!(" {}{origin}", entry.what), body),
    ];
    if let (JournalStatus::Running, Some(progress)) = (entry.status, &entry.progress) {
        spans.push(Span::styled(
            format!(
                "  ({} of {}: {})",
                progress.done + 1,
                progress.of,
                progress.area
            ),
            body.fg(Color::Yellow),
        ));
    }
    if let Some(duration) = entry.took {
        spans.push(Span::styled(
            format!("  ({})", took(duration)),
            body.fg(DIM),
        ));
    }
    Line::from(spans)
}

/// What the chosen entry says about itself, in words.
pub(super) fn detail_text(entry: &Activity) -> String {
    let went = went_text(entry);
    match (&entry.way_back, &entry.id) {
        (Some(way_back), Some(id)) => format!("{went}\n{}", way_back.words(id).join("\n")),
        _ => went,
    }
}

pub(super) fn went_text(entry: &Activity) -> String {
    match entry.status {
        JournalStatus::Done if entry.detail.is_empty() => "It went through.".to_owned(),
        JournalStatus::Done | JournalStatus::Interrupted => entry.detail.clone(),
        JournalStatus::Failed => format!("It failed: {}", entry.detail),
        JournalStatus::Cancelled => format!("It was stopped on request. {}", entry.detail),
        JournalStatus::Running => running_text(entry),
    }
}

/// What a change that is running says about itself, and what can be done about it.
pub(super) fn running_text(entry: &Activity) -> String {
    let mut text = String::from("It is being done right now");
    if let Some(pid) = entry.pid {
        text.push_str(&format!(" by process {pid}"));
    }
    text.push('.');
    if let Some(progress) = &entry.progress {
        text.push_str(&format!(
            " It is on step {} of {}: {}.",
            progress.done + 1,
            progress.of,
            progress.area
        ));
    }
    text.push_str(
        " Closing this window does not stop it. x asks it to stop after the area it is on.",
    );
    text
}

pub(super) fn render_detail(state: &State, frame: &mut Frame, area: Rect) {
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
