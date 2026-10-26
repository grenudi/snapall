// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tab bar: the brand badge, the three tabs, and where a click on each lands.

use crate::view::*;

/// The brand badge that opens the header: the tabs start after it.
pub(crate) const BRAND_WIDTH: u16 = 9;

pub(crate) fn tab_label(tab: Tab) -> String {
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

pub(crate) fn tab_spans(state: &State) -> Vec<Span<'static>> {
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

pub(crate) fn render_header(state: &State, frame: &mut Frame, area: Rect) {
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
        let seconds = state.busy_ticks / 10;
        let elapsed = if seconds > 0 {
            format!(" {seconds} s")
        } else {
            String::new()
        };
        spans.push(Span::styled(
            format!(
                "   {} {word}{elapsed}",
                FRAMES[state.spinner % FRAMES.len()]
            ),
            Style::new().fg(Color::Yellow).bold(),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
