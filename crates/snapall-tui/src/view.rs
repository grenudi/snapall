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
use snapall_core::explain;
use snapall_core::{ChangeKind, ConfigName, FileState, SnapshotId, SnapshotInfo, SnapshotKind};

use crate::model::{Button, Click, Load, Modal, Notice, Pane, RestoreStage, State, Target};

use crate::tab::Tab;
use crate::theme::{ACCENT, DIM};
use crate::{status_bar, tabs};

/// Where everything is, for a terminal of this size. Drawing and clicking both start here, so a
/// click can only ever land on what was drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Areas {
    /// The tab bar.
    pub header: Rect,
    /// Everything between the tab bar and the status bar.
    pub body: Rect,
    pub configs: Rect,
    pub snapshots: Rect,
    /// The status bar, under every view.
    pub status: Rect,
    pub notice: Rect,
    pub keys: Rect,
}

pub fn areas(size: Rect) -> Areas {
    let [header, body, status, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(size);
    let [configs, snapshots] =
        Layout::horizontal([Constraint::Length(28), Constraint::Min(20)]).areas(body);
    let [notice, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(footer);
    Areas {
        header,
        body,
        configs,
        snapshots,
        status,
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
    match (state.file.is_some(), state.tab) {
        (true, _) => render_file(state, frame, areas.snapshots),
        (false, Tab::Snapshots) => {
            render_configs(state, frame, areas.configs);
            render_snapshots(state, frame, areas.snapshots);
        }
        (false, Tab::Health) => tabs::health::render(state, frame, areas.body),
        (false, Tab::Activity) => tabs::activity::render(state, frame, areas.body),
    }
    status_bar::render(state, frame, areas.status);
    render_footer(state, frame, &areas);

    if let Some(modal) = &state.modal {
        render_modal(modal, state, frame);
    }
}

/// The brand badge that opens the header: the tabs start after it.
const BRAND_WIDTH: u16 = 9;

fn tab_label(tab: Tab) -> String {
    format!(" {} {} ", tab.key(), tab.title())
}

/// Each tab's name in the header and the cells it covers. What does not fit is not drawn.
pub fn tab_rects(header: Rect) -> Vec<(Rect, Tab)> {
    let mut x = header.x.saturating_add(BRAND_WIDTH).saturating_add(1);
    let mut out = Vec::new();
    for tab in Tab::ALL {
        let width = u16::try_from(tab_label(tab).chars().count()).unwrap_or(u16::MAX);
        if x.saturating_add(width) > header.x.saturating_add(header.width) {
            break;
        }
        out.push((
            Rect {
                x,
                y: header.y,
                width,
                height: 1.min(header.height),
            },
            tab,
        ));
        x = x.saturating_add(width).saturating_add(1);
    }
    out
}

fn tab_spans(state: &State) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw(" ")];
    for (_, tab) in tab_rects(Rect {
        x: 0,
        y: 0,
        width: u16::MAX,
        height: 1,
    }) {
        let style = if tab == state.tab {
            Style::new().fg(Color::Black).bg(Color::White).bold()
        } else {
            Style::new().fg(DIM)
        };
        spans.push(Span::styled(tab_label(tab), style));
        spans.push(Span::raw(" "));
    }
    spans
}

fn render_header(state: &State, frame: &mut Frame, area: Rect) {
    let mut spans = vec![Span::styled(
        " snapall ",
        Style::new().fg(Color::Black).bg(ACCENT).bold(),
    )];
    match &state.file {
        Some(view) => spans.push(Span::styled(
            format!("  history of {}", view.target.path),
            Style::new().fg(DIM),
        )),
        None => spans.extend(tab_spans(state)),
    }
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
const ACTIONS: [(Button, &str, &str); 9] = [
    (Button::Create, "c", "create"),
    (Button::CreateAll, "a", "all"),
    (Button::Delete, "d", "delete"),
    (Button::Restore, "u", "restore"),
    (Button::Points, "R", "points"),
    (Button::Reload, "r", "reload"),
    (Button::Protect, "p", "protect"),
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
        // one space between buttons: nine of them must fit 80 columns, and the key letter is bold
        x = x.saturating_add(width).saturating_add(1);
    }
    out
}

/// What a button does, in plain words, for the footer line while the mouse is over it. The same
/// button means different things in different prompts, so the prompt it is in is taken into account.
pub fn button_help(button: Button, state: &State) -> &'static str {
    match (&state.modal, button) {
        (Some(Modal::Describe { .. }), Button::Confirm) => "Take the snapshot now",
        (Some(Modal::ConfirmDelete { .. }), Button::Confirm) => {
            "Delete them for good (your files are not touched)"
        }
        (Some(Modal::Restore { .. }), Button::Confirm) => {
            "Put the files back (how they are now is saved first, so this can be undone)"
        }
        (Some(Modal::Help { .. }), Button::Confirm) => "Show the other page",
        (Some(Modal::Protect { .. }), Button::Confirm) => {
            "Set this part of the disk up for snapshots (nothing is copied or moved)"
        }
        (Some(Modal::Protect { .. }), Button::Next) => "Show the next part of the disk",
        (Some(Modal::Points { .. }), Button::Confirm) => {
            "Put every area of the chosen restore point back (not the system)"
        }
        (Some(Modal::Restore { .. }), Button::RestoreAll) => {
            "Put all areas of this restore point back at once (not the system)"
        }
        (Some(_), Button::Cancel) => "Close this without changing anything",
        (Some(_), _) => "",
        (None, Button::Create) => {
            "Take a snapshot of the selected area now: a saved moment you can go back to"
        }
        (None, Button::CreateAll) => "Take a snapshot of every protected area at once",
        (None, Button::Delete) => {
            "Delete the marked snapshots for good. Your files are not touched"
        }
        (None, Button::Restore) if state.file.is_some() => {
            "Put this file back to how this snapshot had it (asks first)"
        }
        (None, Button::Restore) => {
            "Go back to the selected snapshot: shows what changes and asks first"
        }
        (None, Button::Reload) => "Look again at which snapshots exist",
        (None, Button::Points) => "See the restore points and put every area back to one",
        (None, Button::Protect) => {
            "See which parts of your disk have no snapshots yet, and protect them"
        }
        (None, Button::Help) => "Explain what snapshots are and what each key does",
        (None, Button::Quit) => "Close snapall. Nothing is changed",
        (None, Button::Confirm | Button::Cancel | Button::Next | Button::RestoreAll) => "",
    }
}

fn render_footer(state: &State, frame: &mut Frame, areas: &Areas) {
    let explanation = state
        .hover
        .map(|button| button_help(button, state))
        .filter(|text| !text.is_empty());
    // What deserves attention comes before a tip: a part of the disk without snapshots, then the boot menu.
    let attention = state
        .context
        .protect_hint
        .as_ref()
        .filter(|_| state.offers.iter().any(|offer| offer.recommended))
        .or(state.context.boot_tip.as_ref());
    let notice = match (explanation, &state.notice, attention) {
        // Under the mouse: what the button does.
        (Some(text), _, _) => Span::styled(format!(" {text}"), Style::new().fg(Color::Cyan)),
        (None, Some(Notice::Info(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Green))
        }
        (None, Some(Notice::Error(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Red))
        }
        // The tip stays until something else needs the line.
        (None, None, Some(tip)) => Span::styled(format!(" {tip}"), Style::new().fg(Color::Yellow)),
        (None, None, None) => Span::raw(""),
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
        Modal::Help { page } => help_spec(*page),
        Modal::Protect { index } => protect_spec(*index, state),
        Modal::Points { index } => points_spec(*index, state),
    }
}

/// The prompt for a new snapshot's description.
fn describe_spec(target: &Target, input: &str, error: Option<&str>, state: &State) -> ModalSpec {
    let title = match target {
        Target::One(config) => format!(" New snapshot of {config} "),
        Target::All => {
            let names: Vec<String> = state.entries().iter().map(|e| e.name.to_string()).collect();
            format!(
                " New snapshot of ALL protected areas ({}) ",
                names.join(", ")
            )
        }
    };
    let mut lines = vec![
        Line::from("A short note to help you recognise this snapshot later (Enter alone uses \"snapall\"):"),
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
        Line::from("Your files are not touched: only the saved moments are removed, and the space they held is freed."),
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

/// The restore prompt, and, when the snapshot is one of several taken together, the offer to put all
/// of them back.
fn restore_spec(
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    stage: &RestoreStage,
    state: &State,
) -> ModalSpec {
    let spec = restore_stage_spec(config, id, path, stage, state);
    if !matches!(stage, RestoreStage::Ready(_)) {
        return spec;
    }
    match &state.point_offer {
        Some((c, i, offer)) if c == config && *i == id && offer.areas_to_change() > 0 => {
            with_point(spec, offer)
        }
        _ => spec,
    }
}

/// Add the restore point's offer to a prompt: what the point is, and the button that puts it all back.
fn with_point(mut spec: ModalSpec, offer: &crate::model::PointOffer) -> ModalSpec {
    let areas: Vec<&str> = offer
        .point
        .members
        .iter()
        .map(|m| m.config.as_str())
        .collect();
    let k = offer.areas_to_change();
    spec.lines.push(Line::from(""));
    spec.lines.push(Line::from(format!(
        "This snapshot is part of restore point {} ({}), taken in {} areas: {}.",
        offer.point.point,
        offer.point.description,
        areas.len(),
        areas.join(", ")
    )));
    spec.lines.push(Line::styled(
        format!("Press A to put all {k} that differ back at once."),
        Style::new().fg(Color::Yellow),
    ));
    spec.lines.push(Line::styled(
        "The running system is left alone.",
        Style::new().fg(Color::Yellow),
    ));
    spec.height += 6;
    let last = spec.buttons.len().saturating_sub(1);
    spec.buttons
        .insert(last, (Button::RestoreAll, format!("A: Restore all {k}")));
    spec
}

/// The restore prompt: looking, failed, the system root (refused), nothing to restore, or ready.
fn restore_stage_spec(
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

/// The restore points: the list with the cursor on one, and what putting that one back would do.
fn points_spec(index: usize, state: &State) -> ModalSpec {
    let simple = |lines: Vec<Line<'static>>, height: u16| ModalSpec {
        title: " Restore points ".into(),
        lines,
        color: ACCENT,
        width: 78,
        height,
        buttons: vec![close_button()],
    };
    let Some(points) = &state.points else {
        return simple(
            vec![Line::styled(
                "Looking for restore points...",
                Style::new().fg(DIM),
            )],
            8,
        );
    };
    if points.is_empty() {
        return simple(
            vec![
                Line::from("There are no restore points yet."),
                Line::from(""),
                Line::from("Taking a snapshot of every area at once (press a, or run sudo snapall) makes one: the snapshots one action takes together, so that everything can be put back to that moment in one step."),
            ],
            11,
        );
    }
    let index = index.min(points.len() - 1);
    let mut lines = vec![
        Line::from("Choose a restore point (Up and Down):"),
        Line::from(""),
    ];
    let first = index.saturating_sub(5);
    for (at, point) in points.iter().enumerate().skip(first).take(6) {
        let marker = if at == index { "> " } else { "  " };
        let note = if point.tagged {
            ""
        } else {
            " (older: matched by name and time)"
        };
        let row = format!(
            "{marker}{}  {}  {}{note}",
            point.point,
            point.date.as_deref().unwrap_or(""),
            point.description
        );
        let row: String = row.chars().take(74).collect();
        lines.push(if at == index {
            Line::styled(row, Style::new().fg(Color::Yellow).bold())
        } else {
            Line::styled(row, Style::new().fg(DIM))
        });
    }
    lines.push(Line::from(""));
    let preview = state
        .point_preview
        .as_ref()
        .filter(|p| p.point.point == points[index].point);
    match preview {
        None => lines.push(Line::styled(
            "Looking at what it would change...",
            Style::new().fg(DIM),
        )),
        Some(preview) => {
            lines.push(Line::from("Putting every area back would:"));
            let width = preview
                .views
                .iter()
                .map(|v| v.config.as_str().len())
                .max()
                .unwrap_or(0);
            for view in &preview.views {
                let what = match &view.view {
                    snapall_core::MemberView::Changes(p) if p.changes.is_empty() => {
                        "nothing: it already matches".to_owned()
                    }
                    snapall_core::MemberView::Changes(p) => {
                        let (changed, added, removed) = restore_counts(&p.changes);
                        format!("change {} path(s): {changed} reverted, {added} removed, {removed} brought back", p.changes.len())
                    }
                    snapall_core::MemberView::SystemRoot => {
                        format!(
                            "left alone: the running system is not put back (its snapshot here is #{})",
                            view.id
                        )
                    }
                    snapall_core::MemberView::Failed(why) => {
                        format!("could not be looked at: {why}")
                    }
                };
                lines.push(Line::from(format!("  {:<width$}  {what}", view.config)));
            }
        }
    }
    let mut buttons = Vec::new();
    if let Some(preview) = preview.filter(|p| p.areas_to_change() > 0) {
        buttons.push(confirm_button(&format!(
            "Restore all {}",
            preview.areas_to_change()
        )));
    }
    buttons.push(close_button());
    let height = u16::try_from(lines.len() + 3).unwrap_or(20).clamp(10, 21);
    ModalSpec {
        title: format!(" Restore points  ({} of {}) ", index + 1, points.len()),
        lines,
        color: Color::Yellow,
        width: 78,
        height,
        buttons,
    }
}

/// The offer to protect a part of the disk: one at a time, with what it holds and what will happen.
fn protect_spec(index: usize, state: &State) -> ModalSpec {
    let count = state.offers.len();
    let Some(offer) = state.offers.get(index) else {
        return ModalSpec {
            title: " Protect ".into(),
            lines: vec![
                Line::from("Everything snapall recommends protecting has snapshots already."),
                Line::from(""),
                Line::from(explain::NOT_A_BACKUP),
            ],
            color: ACCENT,
            width: 72,
            height: 10,
            buttons: vec![close_button()],
        };
    };
    let mut lines: Vec<Line> = offer
        .summary
        .lines()
        .map(|l| Line::from(l.to_owned()))
        .collect();
    let mut buttons = Vec::new();
    if state.context.administrator {
        buttons.push(confirm_button("Protect it"));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "To protect it, close this and start snapall with administrator rights:",
            Style::new().fg(Color::Yellow),
        ));
        lines.push(Line::styled(
            "    sudo snapall tui",
            Style::new().fg(Color::Yellow).bold(),
        ));
    }
    if count > 1 {
        buttons.push((Button::Next, "Tab: Next".to_owned()));
    }
    buttons.push(if state.context.administrator {
        cancel_button()
    } else {
        close_button()
    });
    ModalSpec {
        title: format!(" Protect  ({} of {count}) ", index + 1),
        lines,
        color: Color::Yellow,
        width: 78,
        height: 20,
        buttons,
    }
}

/// The help. Page one teaches what snapall is about, for someone who knows nothing of it; page two
/// says what each key and button does.
fn help_spec(page: usize) -> ModalSpec {
    let (title, lines) = if page == 0 {
        (" What is this?  (1 of 2) ", concepts_page())
    } else {
        (" Keys and buttons  (2 of 2) ", keys_page())
    };
    ModalSpec {
        title: title.into(),
        lines,
        color: ACCENT,
        width: 84,
        height: 22,
        buttons: vec![confirm_button("Other page"), close_button()],
    }
}

fn concepts_page() -> Vec<Line<'static>> {
    let term = |name: &'static str, meaning: &'static str| {
        vec![
            Line::from(Span::styled(name, Style::new().fg(ACCENT).bold())),
            Line::from(format!("    {meaning}")),
        ]
    };
    let mut lines: Vec<Line> = explain::WHAT_IS_A_SNAPSHOT
        .split("\n\n")
        .flat_map(|paragraph| [Line::from(paragraph.to_owned()), Line::from("")])
        .collect();
    lines.extend(term(
        "Protected area (the list on the left)",
        "A part of your disk that snapshots are kept for, such as System or Home.",
    ));
    lines.extend(term(
        "Snapshot (the list on the right)",
        "One saved moment of that area. Newest first.",
    ));
    lines.extend(term("Restore", explain::RESTORING));
    lines
}

fn keys_page() -> Vec<Line<'static>> {
    let rows = [
        ("up/down  k/j", "Move in the list that is selected"),
        ("left/right  Tab", "Switch between areas and snapshots"),
        ("space", "Mark a snapshot (Delete can take several)"),
        ("c", "Take a snapshot of the selected area now"),
        ("a", "Take a snapshot of every protected area"),
        ("d", "Delete marked snapshots for good (files untouched)"),
        (
            "u",
            "Restore: go back to the selected snapshot (asks first)",
        ),
        ("A", "In Restore: put its whole restore point back"),
        ("p", "Protect: see what has no snapshots yet, and set it up"),
        ("R", "Restore points: put every area back to one moment"),
        ("r", "Look again at which snapshots exist"),
        ("q  Ctrl-C", "Close snapall (changes nothing)"),
        (
            "mouse",
            "Click anything; hover a button to read what it does",
        ),
    ];
    rows.iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(format!("{key:<20}"), Style::new().fg(ACCENT).bold()),
                Span::raw(*what),
            ])
        })
        .collect()
}

/// Where a prompt is and where each of its buttons is, for a terminal of this size.
fn modal_geometry(
    modal: &Modal,
    state: &State,
    size: Rect,
) -> (ModalSpec, Rect, Vec<(Rect, Button, String)>) {
    let spec = modal_spec(modal, state);
    // A prompt never covers the two footer lines: the one below it explains the button the mouse is
    // over, and a prompt is exactly where that explanation matters.
    let above_footer = Rect {
        height: size.height.saturating_sub(3),
        ..size
    };
    let area = centered(above_footer, spec.width, spec.height);
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
            Paragraph::new("no snapshots of this area yet. Press q to quit.")
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
    if state.file.is_none() {
        if let Some((_, tab)) = tab_rects(areas.header)
            .into_iter()
            .find(|(rect, _)| contains(*rect, col, row))
        {
            return Some(Click::Tab(tab));
        }
    }
    if let Some(data) = &state.health.data {
        if let Some((_, topic)) = status_bar::chip_rects(areas.status, &data.report.chips)
            .into_iter()
            .find(|(rect, _)| contains(*rect, col, row))
        {
            return Some(Click::Chip(topic));
        }
    }
    if let Some((_, button, _, _)) = footer_buttons(areas.keys, state.file.is_some())
        .into_iter()
        .find(|(rect, _, _, _)| contains(*rect, col, row))
    {
        return Some(Click::Button(button));
    }
    if state.file.is_none() && state.tab != Tab::Snapshots {
        let index = match state.tab {
            Tab::Health => tabs::health::row_at(state, areas.body, row),
            Tab::Activity => tabs::activity::row_at(state, areas.body, row),
            Tab::Snapshots => None,
        };
        return index.map(|index| Click::TabRow { index });
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
