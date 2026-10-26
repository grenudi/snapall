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
use snapall_core::explain::{cleanup_from, cleanup_text, Cleanup};
use snapall_core::health::format_bytes;
use snapall_core::{AreaFacts, Facts, Health, Level, Remedy, Topic};

use crate::model::{Effect, Key, Notice, State};
use crate::theme::{mark_of, of_level, ACCENT, DIM};

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

// ---- what the rows say --------------------------------------------------------------------------

/// The groups the rows fall into, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    NeedsALook,
    GoodToKnow,
    Space,
    Cleanup,
    BootMenu,
}

impl Section {
    fn title(self) -> &'static str {
        match self {
            Section::NeedsALook => "Needs a look",
            Section::GoodToKnow => "Good to know",
            Section::Space => "Space",
            Section::Cleanup => "Automatic cleanup",
            Section::BootMenu => "Boot menu",
        }
    }
}

/// One thing on the screen: what it is called, how it stands, and what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub section: Section,
    pub topic: Topic,
    pub level: Level,
    pub label: String,
    /// What it is, why it matters, what to do: shown beneath the list when the row is chosen.
    pub detail: String,
    pub remedy: Option<Remedy>,
}

/// Everything the tab lists, in the order it is shown.
pub fn rows(data: &HealthData) -> Vec<Row> {
    let mut rows = finding_rows(&data.report);
    rows.extend(data.facts.areas.iter().map(space_row));
    rows.push(cleanup_row(&data.facts));
    rows.push(boot_menu_row(&data.facts));
    rows
}

fn finding_rows(report: &Health) -> Vec<Row> {
    report
        .findings
        .iter()
        .map(|finding| Row {
            section: if finding.level >= Level::Attention {
                Section::NeedsALook
            } else {
                Section::GoodToKnow
            },
            topic: finding.topic,
            level: finding.level,
            label: finding.headline.clone(),
            detail: finding.why.clone(),
            remedy: finding.remedy,
        })
        .collect()
}

fn space_row(area: &AreaFacts) -> Row {
    let room = match area.measured_bytes {
        Some(bytes) => format!("{} held", format_bytes(bytes)),
        None => "room not measured".to_owned(),
    };
    let snapshots = format!(
        "{} snapshot{}",
        area.snapshots,
        if area.snapshots == 1 { "" } else { "s" }
    );
    let disk = area
        .disk
        .map_or(String::new(), |d| format!("  disk {}%", d.percent_used()));
    let level = match area.disk.map(|d| d.percent_used()) {
        Some(95..) => Level::Problem,
        Some(85..=94) => Level::Attention,
        _ => Level::Good,
    };
    Row {
        section: Section::Space,
        topic: Topic::Space,
        level,
        label: format!(
            "{} ({})  {snapshots}  {room}{disk}",
            area.title(),
            area.subvolume
        ),
        detail: space_detail(area, &snapshots),
        remedy: None,
    }
}

fn space_detail(area: &AreaFacts, snapshots: &str) -> String {
    let mut text = format!(
        "{} keeps its snapshots in {}. It has {snapshots}",
        area.title(),
        area.lives_in()
    );
    if let Some(oldest) = &area.oldest {
        text.push_str(&format!(", the oldest from {oldest}"));
    }
    text.push_str(". ");
    text.push_str(&match area.measured_bytes {
        Some(bytes) => format!(
            "Together they hold {}, room that nothing else uses. ",
            format_bytes(bytes)
        ),
        None => "The room they take is not measured (see the note about quotas). ".to_owned(),
    });
    if let Some(disk) = area.disk {
        text.push_str(&format!(
            "The disk it is on is {}% full. ",
            disk.percent_used()
        ));
    }
    if let Some(keep) = area.retention.as_ref().and_then(|r| r.keep) {
        text.push_str(&format!(
            "It keeps the newest {keep}{}.",
            minutes_note(area.retention.as_ref().and_then(|r| r.min_age_seconds))
        ));
    }
    text
}

/// ", and never cleans up one younger than 30 minutes".
fn minutes_note(seconds: Option<u64>) -> String {
    seconds.map_or(String::new(), |s| {
        format!(", and never cleans up one younger than {} minutes", s / 60)
    })
}

fn cleanup_state(facts: &Facts) -> Cleanup {
    cleanup_from(
        facts.cleanup_timer.enabled.as_deref(),
        facts.cleanup_timer.active.as_deref(),
    )
}

fn cleanup_row(facts: &Facts) -> Row {
    let state = cleanup_state(facts);
    let (level, word, remedy) = match state {
        Cleanup::On => (Level::Good, "on", None),
        Cleanup::Off => (Level::Attention, "off", Some(Remedy::SwitchOnCleanup)),
        Cleanup::Unavailable => (Level::Note, "could not be found out", None),
    };
    let mut detail = cleanup_text(state).to_owned();
    for area in &facts.areas {
        if let Some(keep) = area.retention.as_ref().and_then(|r| r.keep) {
            detail.push_str(&format!(
                " {} keeps the newest {keep}{}.",
                area.title(),
                minutes_note(area.retention.as_ref().and_then(|r| r.min_age_seconds))
            ));
        }
    }
    Row {
        section: Section::Cleanup,
        topic: Topic::Cleanup,
        level,
        label: format!("Cleaning up old snapshots: {word}"),
        detail,
        remedy,
    }
}

fn boot_menu_row(facts: &Facts) -> Row {
    let sync = &facts.boot_menu_sync;
    let (level, label, detail) = if facts.boot_menu_plugin {
        (
            Level::Good,
            "Boot menu: kept up to date by a plugin",
            "A plugin of the snapshot tool adds each new snapshot to the boot menu, so you can start the computer from an earlier one.",
        )
    } else if sync.is_running() {
        (
            Level::Good,
            "Boot menu: following your snapshots",
            "The tool that adds your snapshots to the boot menu (limine-snapper-sync) is running, so you can start the computer from an earlier one. Only the system's own snapshots appear there, not the other areas.",
        )
    } else if sync.is_installed() {
        (
            Level::Attention,
            "Boot menu: not following your snapshots",
            "The tool that adds your snapshots to the boot menu (limine-snapper-sync) is installed but not running, so new snapshots do not reach the boot menu. Start it with: systemctl enable --now limine-snapper-sync.service",
        )
    } else {
        (
            Level::Good,
            "Boot menu: not used on this computer",
            "Your snapshots are not listed in a boot menu. That only matters if you want to start the computer from an earlier snapshot of the system.",
        )
    };
    Row {
        section: Section::BootMenu,
        topic: Topic::BootMenu,
        level,
        label: label.to_owned(),
        detail: detail.to_owned(),
        remedy: None,
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
        Key::Home | Key::Char('g') => state.health.cursor = 0,
        Key::End | Key::Char('G') => state.health.cursor = last,
        Key::Enter => return Some(act_on_chosen(state)),
        _ => return None,
    }
    Some(Vec::new())
}

/// Enter on a row that has something snapall can do about it.
fn act_on_chosen(state: &mut State) -> Vec<Effect> {
    let chosen = state.health.rows().into_iter().nth(state.health.cursor);
    match chosen.and_then(|row| row.remedy) {
        Some(Remedy::SwitchOnCleanup) if !state.context.administrator => {
            state.notice = Some(Notice::Info(
                "Switching cleanup on needs administrator rights: start snapall with sudo.".into(),
            ));
            Vec::new()
        }
        Some(Remedy::SwitchOnCleanup) => {
            state.notice = Some(Notice::Info("switching automatic cleanup on".into()));
            vec![Effect::SwitchOnCleanup]
        }
        None => Vec::new(),
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
    let mut lines = Vec::new();
    if let Some(row) = rows.get(state.health.cursor) {
        lines.push(Line::from(row.detail.clone()));
        if row.remedy == Some(Remedy::SwitchOnCleanup) {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Enter: switch automatic cleanup on",
                Style::new()
                    .fg(ratatui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(DIM))
        .title(Span::styled(" What this means ", Style::new().fg(DIM)));
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(block),
        area,
    );
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
