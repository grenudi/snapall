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
use snapall_core::{ChangeKind, ConfigName, FileState, SnapshotId, SnapshotInfo, SnapshotKind};

use crate::model::{Button, Click, Load, Modal, Notice, Pane, RestoreStage, State, Target};

const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

/// Where everything is, for a terminal of this size. Drawing and clicking both start here, so a
/// click can only ever land on what was drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Areas {
    pub header: Rect,
    pub configs: Rect,
    pub snapshots: Rect,
    pub notice: Rect,
    pub keys: Rect,
}

pub fn areas(size: Rect) -> Areas {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(size);
    let [configs, snapshots] =
        Layout::horizontal([Constraint::Length(28), Constraint::Min(20)]).areas(body);
    let [notice, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(footer);
    Areas {
        header,
        configs,
        snapshots,
        notice,
        keys,
    }
}

/// [`areas`], adjusted for what is on screen: when the UI is open on one file, its versions take the
/// whole body and there is no configs pane.
pub fn layout(state: &State, size: Rect) -> Areas {
    let mut areas = areas(size);
    if state.file.is_some() {
        areas.snapshots = Rect {
            x: areas.configs.x,
            y: areas.configs.y,
            width: areas.configs.width.saturating_add(areas.snapshots.width),
            height: areas.configs.height,
        };
        areas.configs = Rect {
            width: 0,
            height: 0,
            ..areas.configs
        };
    }
    areas
}

pub fn render(state: &State, frame: &mut Frame) {
    let areas = layout(state, frame.area());
    render_header(state, frame, areas.header);
    if state.file.is_some() {
        render_file(state, frame, areas.snapshots);
    } else {
        render_configs(state, frame, areas.configs);
        render_snapshots(state, frame, areas.snapshots);
    }
    render_footer(state, frame, &areas);

    if let Some(modal) = &state.modal {
        render_modal(modal, state, frame);
    }
}

fn render_header(state: &State, frame: &mut Frame, area: Rect) {
    let mut spans = vec![
        Span::styled(" snapall ", Style::new().fg(Color::Black).bg(ACCENT).bold()),
        Span::styled(
            match &state.file {
                Some(view) => format!("  history of {}", view.target.path),
                None => "  snapper snapshots".to_owned(),
            },
            Style::new().fg(DIM),
        ),
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

/// The footer's buttons, in order: the button, the key it stands for, and its label.
const ACTIONS: [(Button, &str, &str); 7] = [
    (Button::Create, "c", "create"),
    (Button::CreateAll, "a", "all"),
    (Button::Delete, "d", "delete"),
    (Button::Restore, "u", "restore"),
    (Button::Reload, "r", "reload"),
    (Button::Help, "?", "help"),
    (Button::Quit, "q", "quit"),
];

/// Each footer button and the cells it covers on the key line.
const FILE_ACTIONS: [(Button, &str, &str); 4] = [
    (Button::Restore, "u", "restore this version"),
    (Button::Reload, "r", "reload"),
    (Button::Help, "?", "help"),
    (Button::Quit, "q", "quit"),
];

fn footer_buttons(keys: Rect, file_mode: bool) -> Vec<(Rect, Button, &'static str, &'static str)> {
    let mut x = keys.x.saturating_add(1);
    let mut out = Vec::new();
    let actions: &[(Button, &str, &str)] = if file_mode { &FILE_ACTIONS } else { &ACTIONS };
    for &(button, key, label) in actions {
        let width = (key.chars().count() + 1 + label.chars().count()) as u16;
        if x.saturating_add(width) > keys.x.saturating_add(keys.width) {
            break; // what does not fit is not drawn, and so cannot be clicked
        }
        out.push((
            Rect {
                x,
                y: keys.y,
                width,
                height: 1.min(keys.height),
            },
            button,
            key,
            label,
        ));
        x = x.saturating_add(width).saturating_add(2);
    }
    out
}

fn render_footer(state: &State, frame: &mut Frame, areas: &Areas) {
    let notice = match (&state.notice, &state.context.boot_tip) {
        (Some(Notice::Info(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Green))
        }
        (Some(Notice::Error(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Red))
        }
        // The tip stays until something else needs the line.
        (None, Some(tip)) => Span::styled(format!(" {tip}"), Style::new().fg(Color::Yellow)),
        (None, None) => Span::raw(""),
    };
    frame.render_widget(Paragraph::new(Line::from(notice)), areas.notice);
    for (rect, _button, key, label) in footer_buttons(areas.keys, state.file.is_some()) {
        let line = Line::from(vec![
            Span::styled(key, Style::new().fg(ACCENT).bold()),
            Span::styled(format!(" {label}"), Style::new().fg(DIM)),
        ]);
        frame.render_widget(Paragraph::new(line), rect);
    }
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

/// How a prompt looks and which buttons it has.
struct ModalSpec {
    title: String,
    lines: Vec<Line<'static>>,
    color: Color,
    width: u16,
    height: u16,
    /// The buttons, left to right: the button, and its label.
    buttons: Vec<(Button, String)>,
}

fn restore_counts(changes: &[snapall_core::Change]) -> (usize, usize, usize) {
    let count = |kind| changes.iter().filter(|c| c.kind == kind).count();
    (
        count(ChangeKind::Changed),
        count(ChangeKind::Added),
        count(ChangeKind::Removed),
    )
}

fn confirm_button(label: &str) -> (Button, String) {
    (Button::Confirm, format!("Enter: {label}"))
}

fn cancel_button() -> (Button, String) {
    (Button::Cancel, "Esc: Cancel".to_owned())
}

fn close_button() -> (Button, String) {
    (Button::Cancel, "Esc: Close".to_owned())
}

fn modal_spec(modal: &Modal, state: &State) -> ModalSpec {
    match modal {
        Modal::Describe {
            target,
            input,
            error,
        } => describe_spec(target, input, error.as_deref(), state),
        Modal::ConfirmDelete { config, ids } => delete_spec(config, ids),
        Modal::Restore {
            config,
            id,
            path,
            stage,
        } => restore_spec(config, *id, path.as_deref(), stage, state),
        Modal::Help => help_spec(),
    }
}

/// The prompt for a new snapshot's description.
fn describe_spec(target: &Target, input: &str, error: Option<&str>, state: &State) -> ModalSpec {
    let title = match target {
        Target::One(config) => format!(" New snapshot of {config} "),
        Target::All => {
            let names: Vec<String> = state.entries().iter().map(|e| e.name.to_string()).collect();
            format!(" New snapshot of ALL configs ({}) ", names.join(", "))
        }
    };
    let mut lines = vec![
        Line::from("Description (Enter alone uses \"snapall\"):"),
        Line::from(vec![
            Span::styled(input.to_owned(), Style::new().bold()),
            Span::styled(
                "_",
                Style::new().fg(ACCENT).add_modifier(Modifier::SLOW_BLINK),
            ),
        ]),
        Line::from(""),
    ];
    if let Some(error) = error {
        lines.push(Line::styled(error.to_owned(), Style::new().fg(Color::Red)));
    }
    ModalSpec {
        title,
        lines,
        color: ACCENT,
        width: 64,
        height: 9,
        buttons: vec![confirm_button("Create"), cancel_button()],
    }
}

fn delete_spec(config: &ConfigName, ids: &[SnapshotId]) -> ModalSpec {
    let list: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
    let lines = vec![
        Line::from(format!("Delete {} snapshot(s) of {config}?", ids.len())),
        Line::styled(list.join(", "), Style::new().bold()),
        Line::from(""),
        Line::styled("This cannot be undone.", Style::new().fg(Color::Yellow)),
    ];
    ModalSpec {
        title: " Delete ".into(),
        lines,
        color: Color::Red,
        width: 56,
        height: 9,
        buttons: vec![confirm_button("Delete"), cancel_button()],
    }
}

/// The restore prompt: looking, failed, the system root (refused), nothing to restore, or ready.
fn restore_spec(
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    stage: &RestoreStage,
    state: &State,
) -> ModalSpec {
    let title = format!(" Restore {config} to #{id} ");
    match stage {
        RestoreStage::Loading => ModalSpec {
            title,
            lines: vec![Line::styled(
                "Looking at what would change...",
                Style::new().fg(DIM),
            )],
            color: Color::Yellow,
            width: 72,
            height: 8,
            buttons: vec![cancel_button()],
        },
        RestoreStage::Failed(error) => ModalSpec {
            title,
            lines: vec![Line::styled(error.clone(), Style::new().fg(Color::Red))],
            color: Color::Red,
            width: 72,
            height: 9,
            buttons: vec![close_button()],
        },
        RestoreStage::Ready(preview) if preview.is_system_root && path.is_none() => ModalSpec {
            title: " Restoring the system itself ".into(),
            lines: vec![
                Line::from(format!(
                    "{config} is the system root, so snapall will not put it back while it runs."
                )),
                Line::from(""),
                Line::from(state.context.restore_route.clone()),
            ],
            color: Color::Yellow,
            width: 78,
            height: 14,
            buttons: vec![close_button()],
        },
        RestoreStage::Ready(preview) if preview.changes.is_empty() => ModalSpec {
            title,
            lines: vec![Line::from(format!(
                "{config} already matches #{id}: there is nothing to restore."
            ))],
            color: ACCENT,
            width: 72,
            height: 8,
            buttons: vec![close_button()],
        },
        RestoreStage::Ready(preview) => ModalSpec {
            title,
            lines: restore_summary(config, id, path, &preview.changes),
            color: Color::Yellow,
            width: 78,
            height: 16,
            buttons: vec![confirm_button("Restore"), cancel_button()],
        },
    }
}

/// What a restore would do: the counts, the first few paths with a verb each, and the safety net.
fn restore_summary(
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    changes: &[snapall_core::Change],
) -> Vec<Line<'static>> {
    let (changed, added, removed) = restore_counts(changes);
    let mut lines = vec![
        Line::from(format!(
            "Put {} back to how #{id} had it: {changed} reverted, {added} removed, {removed} brought back.",
            path.unwrap_or(config.as_str())
        )),
        Line::from(""),
    ];
    for change in changes.iter().take(6) {
        let verb = match change.kind {
            ChangeKind::Changed => "revert    ",
            ChangeKind::Added => "remove    ",
            ChangeKind::Removed => "bring back",
        };
        lines.push(Line::from(vec![
            Span::styled(verb, Style::new().fg(ACCENT)),
            Span::raw(format!(" {}", change.path)),
        ]));
    }
    if changes.len() > 6 {
        lines.push(Line::styled(
            format!("... and {} more", changes.len() - 6),
            Style::new().fg(DIM),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "The current state is snapshotted first, so this can be undone.",
        Style::new().fg(Color::Yellow),
    ));
    lines
}

/// The key reference.
fn help_spec() -> ModalSpec {
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
        ("u", "restore the config to the snapshot under the cursor"),
        ("r", "reload from snapper"),
        ("q  Ctrl-C", "quit"),
        (
            "mouse",
            "click a row to select, the first column to mark, the wheel to scroll",
        ),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(format!("{key:<22}"), Style::new().fg(ACCENT).bold()),
                Span::raw(*what),
            ])
        })
        .collect();
    ModalSpec {
        title: " Keys ".into(),
        lines,
        color: ACCENT,
        width: 84,
        height: 16,
        buttons: vec![close_button()],
    }
}

/// Where a prompt is and where each of its buttons is, for a terminal of this size.
fn modal_geometry(
    modal: &Modal,
    state: &State,
    size: Rect,
) -> (ModalSpec, Rect, Vec<(Rect, Button, String)>) {
    let spec = modal_spec(modal, state);
    let area = centered(size, spec.width, spec.height);
    let inner = Block::default().borders(Borders::ALL).inner(area);
    // The last inner line holds the buttons; the text above it is cut if the terminal is small.
    let y = inner.y.saturating_add(inner.height.saturating_sub(1));
    let mut x = inner.x.saturating_add(1);
    let mut buttons = Vec::new();
    for (button, label) in &spec.buttons {
        let width = (label.chars().count() + 4) as u16; // "[ " + label + " ]"
        if inner.height == 0 || x.saturating_add(width) > inner.x.saturating_add(inner.width) {
            break;
        }
        buttons.push((
            Rect {
                x,
                y,
                width,
                height: 1,
            },
            *button,
            label.clone(),
        ));
        x = x.saturating_add(width).saturating_add(2);
    }
    (spec, area, buttons)
}

fn render_modal(modal: &Modal, state: &State, frame: &mut Frame) {
    let (spec, area, buttons) = modal_geometry(modal, state, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(spec.color))
        .title(Span::styled(
            spec.title.clone(),
            Style::new().fg(spec.color).bold(),
        ))
        .title_alignment(Alignment::Left);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text_area = Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    };
    frame.render_widget(
        Paragraph::new(Text::from(spec.lines)).wrap(Wrap { trim: false }),
        text_area,
    );
    for (rect, button, label) in buttons {
        let style = match button {
            Button::Cancel => Style::new().fg(DIM),
            _ => Style::new().fg(Color::Black).bg(spec.color).bold(),
        };
        frame.render_widget(
            Paragraph::new(Span::styled(format!("[ {label} ]"), style)),
            rect,
        );
    }
}

fn state_style(state: FileState) -> Style {
    match state {
        FileState::Changed => Style::new().fg(Color::Yellow),
        FileState::DeletedSince => Style::new().fg(Color::Red),
        FileState::Same | FileState::AbsentThen => Style::new().fg(DIM),
    }
}

/// The history of one file: a row per snapshot, newest first, saying how the file stood in it.
fn render_file(state: &State, frame: &mut Frame, area: Rect) {
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
            Paragraph::new("no snapshots of this config yet. Press q to quit.")
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

// ---- clicks ---------------------------------------------------------------------------------

/// The first visible row of a list of `len` rows, `rows` high, whose selection is `cursor`. The
/// panes are redrawn from scratch every frame, so the window depends on nothing but these three
/// numbers: the selection is brought into view by the least scroll from the top.
fn window_start(cursor: usize, rows: usize) -> usize {
    if rows > 0 && cursor >= rows {
        cursor + 1 - rows
    } else {
        0
    }
}

fn contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

/// The pane a terminal cell belongs to, for the wheel.
pub fn pane_at(state: &State, size: Rect, col: u16, row: u16) -> Option<Pane> {
    let areas = layout(state, size);
    if contains(areas.configs, col, row) {
        Some(Pane::Configs)
    } else if contains(areas.snapshots, col, row) {
        Some(Pane::Snapshots)
    } else {
        None
    }
}

/// What a left click at (`col`, `row`) hits, for a terminal of this size, or `None`.
///
/// With a prompt open only the prompt's own buttons answer; behind it, nothing does.
pub fn hit_test(state: &State, size: Rect, col: u16, row: u16) -> Option<Click> {
    if let Some(modal) = &state.modal {
        let (_, _, buttons) = modal_geometry(modal, state, size);
        return buttons
            .into_iter()
            .find(|(rect, _, _)| contains(*rect, col, row))
            .map(|(_, button, _)| Click::Button(button));
    }
    let areas = layout(state, size);
    if let Some((_, button, _, _)) = footer_buttons(areas.keys, state.file.is_some())
        .into_iter()
        .find(|(rect, _, _, _)| contains(*rect, col, row))
    {
        return Some(Click::Button(button));
    }
    let block = Block::default().borders(Borders::ALL);
    let configs = block.inner(areas.configs);
    if state.file.is_none() && contains(configs, col, row) && !state.entries().is_empty() {
        let index =
            window_start(state.config_cursor, configs.height as usize) + (row - configs.y) as usize;
        return (index < state.entries().len()).then_some(Click::Row {
            pane: Pane::Configs,
            index,
        });
    }
    let snapshots = block.inner(areas.snapshots);
    if let Some(view) = &state.file {
        // One header line, then a row per version, newest first.
        if contains(snapshots, col, row) && row > snapshots.y {
            let visible = (snapshots.height as usize).saturating_sub(1);
            let index = window_start(view.cursor, visible) + (row - snapshots.y - 1) as usize;
            if index < state.displayed_versions().len() {
                return Some(Click::Row {
                    pane: Pane::Snapshots,
                    index,
                });
            }
        }
        return None;
    }
    // One header line, then the rows.
    if contains(snapshots, col, row) && row > snapshots.y {
        let visible = (snapshots.height as usize).saturating_sub(1);
        let index = window_start(state.snapshot_cursor, visible) + (row - snapshots.y - 1) as usize;
        if index < state.displayed_snapshots().len() {
            // The first two cells are the mark column and its spacing.
            return Some(if col < snapshots.x.saturating_add(2) {
                Click::Mark { index }
            } else {
                Click::Row {
                    pane: Pane::Snapshots,
                    index,
                }
            });
        }
    }
    None
}
