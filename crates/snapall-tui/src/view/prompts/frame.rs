// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The frame every prompt shares: its size and place (clear of the three lines beneath), its buttons, and the drawing of any of them.

use crate::view::*;

/// A rectangle `width` x `height`, centred in `area` and never larger than it.
pub(crate) fn centered(area: Rect, width: u16, height: u16) -> Rect {
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
pub(crate) struct ModalSpec {
    pub(crate) title: String,
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) color: Color,
    pub(crate) width: u16,
    pub(crate) height: u16,
    /// The buttons, left to right: the button, and its label.
    pub(crate) buttons: Vec<(Button, String)>,
}

pub(crate) fn restore_counts(changes: &[snapall_core::Change]) -> (usize, usize, usize) {
    let count = |kind| changes.iter().filter(|c| c.kind == kind).count();
    (
        count(ChangeKind::Changed),
        count(ChangeKind::Added),
        count(ChangeKind::Removed),
    )
}

/// How many screen rows these lines take inside a prompt of `width`, counting the wrapping of long ones:
/// a prompt sized by its number of lines alone is cut off as soon as one of them wraps.
pub(crate) fn rows_needed(lines: &[Line], width: u16) -> u16 {
    let inner = usize::from(width.saturating_sub(2)).max(1);
    let rows: usize = lines
        .iter()
        .map(|line| line.width().div_ceil(inner).max(1))
        .sum();
    u16::try_from(rows).unwrap_or(u16::MAX)
}

pub(crate) fn confirm_button(label: &str) -> (Button, String) {
    (Button::Confirm, format!("Enter: {label}"))
}

pub(crate) fn cancel_button() -> (Button, String) {
    (Button::Cancel, "Esc: Cancel".to_owned())
}

/// The most rows a prompt may take: the status bar, the line of explanation and the two rows of buttons stay
/// visible under it.
pub(crate) const MAX_PROMPT_HEIGHT: u16 = 20;

pub(crate) fn close_button() -> (Button, String) {
    (Button::Cancel, "Esc: Close".to_owned())
}

pub(crate) fn modal_spec(modal: &Modal, state: &State) -> ModalSpec {
    match modal {
        Modal::Describe {
            target,
            input,
            error,
        } => describe_spec(target, input, error.as_deref(), state),
        Modal::ConfirmDelete { area: config, ids } => delete_spec(config, ids),
        Modal::Restore {
            area: config,
            id,
            path,
            stage,
        } => restore_spec(config, *id, path.as_deref(), stage, state),
        Modal::Help { page } => help_spec(*page),
        Modal::Protect { index } => protect_spec(*index, state),
        Modal::Points { index } => points_spec(*index, state),
        Modal::Job { unit } => job_spec(unit, state),
        Modal::Quotas { turn_on } => quotas_spec(*turn_on, state),
        Modal::StopChange { what, .. } => stop_spec(what),
        Modal::Groups(screen) => groups_spec(screen, state),
        Modal::Limits(editor) => limits_spec(editor, state),
        Modal::StopProtecting(prompt) => unprotect_spec(prompt),
        Modal::Confirm(question) => confirm_spec(question),
    }
}

/// Where a prompt is and where each of its buttons is, for a terminal of this size.
pub(crate) fn modal_geometry(
    modal: &Modal,
    state: &State,
    size: Rect,
) -> (ModalSpec, Rect, Vec<(Rect, Button, String)>) {
    let spec = modal_spec(modal, state);
    // A prompt never covers the footer: the line there explains the button the mouse is over, and a prompt
    // is exactly where that explanation matters.
    let above_footer = Rect {
        height: size.height.saturating_sub(4),
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

pub(crate) fn render_modal(modal: &Modal, state: &State, frame: &mut Frame) {
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
        let mood = look::mood(state, Click::Button(button));
        let style = look::prompt_button(mood, button != Button::Cancel, spec.color);
        frame.render_widget(
            Paragraph::new(Span::styled(format!("[ {label} ]"), style)),
            rect,
        );
    }
}
