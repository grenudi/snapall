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
    for tab in Tab::ALL {
        let style = look::tab(look::mood(state, Click::Tab(tab)), tab == state.tab);
        spans.push(Span::styled(tab_label(tab), style));
        spans.push(Span::raw(" "));
    }
    spans
}

/// The brand badge that opens the header.
fn brand_badge() -> Span<'static> {
    Span::styled(" snapall ", Style::new().fg(Color::Black).bg(ACCENT).bold())
}

/// While work is in flight on the worker thread: a spinner, what it is doing, and for how long once it is more than a
/// moment. It keeps the screen moving, so a long change is not mistaken for a stuck window.
fn working_indicator(state: &State) -> Option<Span<'static>> {
    if state.waiting.pending == 0 {
        return None;
    }
    const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    let word = if state.quit { "finishing" } else { "working" };
    let seconds = state.waiting.busy_ticks / 10;
    let elapsed = if seconds > 0 {
        format!(" {seconds} s")
    } else {
        String::new()
    };
    Some(Span::styled(
        format!(
            "   {} {word}{elapsed}",
            FRAMES[state.waiting.spinner % FRAMES.len()]
        ),
        Style::new().fg(Color::Yellow).bold(),
    ))
}

pub(crate) fn render_header(state: &State, frame: &mut Frame, area: Rect) {
    let mut spans = vec![brand_badge()];
    match &state.file {
        Some(view) => spans.push(Span::styled(
            format!("  history of {}", view.target.path),
            Style::new().fg(DIM),
        )),
        None => spans.extend(tab_spans(state)),
    }
    spans.extend(working_indicator(state));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
