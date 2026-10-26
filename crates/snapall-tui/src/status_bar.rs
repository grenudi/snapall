// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status bar: one line under every view with the numbers that matter (how many snapshots, how
//! much room they take, how full the disk is, whether cleanup runs, how many things need a look). Each
//! item is a button: a click goes to the Health tab, at the row that explains it.
//!
//! Its items are the report's own (`snapall_core::Health::chips`), so what the bar says and what the
//! tab explains cannot drift apart.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use snapall_core::{Chip, Topic};

use crate::model::State;
use crate::theme::{of_level, DIM};

const SEPARATOR: &str = " | ";

/// Each item and the cells it covers, left to right. What does not fit is not drawn, so it cannot be
/// clicked.
pub fn chip_rects(area: Rect, chips: &[Chip]) -> Vec<(Rect, Topic)> {
    let mut x = area.x.saturating_add(1);
    let mut out = Vec::new();
    for chip in chips {
        let width = u16::try_from(chip.label.chars().count()).unwrap_or(u16::MAX);
        if x.saturating_add(width) > area.x.saturating_add(area.width) {
            break;
        }
        out.push((
            Rect {
                x,
                y: area.y,
                width,
                height: 1.min(area.height),
            },
            chip.topic,
        ));
        x = x
            .saturating_add(width)
            .saturating_add(u16::try_from(SEPARATOR.len()).unwrap_or(3));
    }
    out
}

pub fn render(state: &State, frame: &mut Frame, area: Rect) {
    let Some(data) = &state.health.data else {
        frame.render_widget(
            Paragraph::new(" checking how protection stands...").style(Style::new().fg(DIM)),
            area,
        );
        return;
    };
    let shown = chip_rects(area, &data.report.chips).len();
    let mut spans = vec![Span::raw(" ")];
    for (index, chip) in data.report.chips.iter().take(shown).enumerate() {
        if index > 0 {
            spans.push(Span::styled(SEPARATOR, Style::new().fg(DIM)));
        }
        spans.push(Span::styled(
            chip.label.clone(),
            Style::new()
                .fg(of_level(chip.level))
                .add_modifier(Modifier::BOLD),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
