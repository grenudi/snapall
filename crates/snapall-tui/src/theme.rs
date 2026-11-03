// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The colours, in one place, so every screen speaks in the same ones: good is green, something to
//! know is cyan, something to do is yellow, something to do now is red.

use ratatui::style::{Color, Modifier, Style};
use snapall_core::Level;

pub const ACCENT: Color = Color::Cyan;
pub const DIM: Color = Color::DarkGray;

/// How a finding or a status-bar item is coloured.
pub fn of_level(level: Level) -> Color {
    match level {
        Level::Good => Color::Green,
        Level::Note => ACCENT,
        Level::Attention => Color::Yellow,
        Level::Problem => Color::Red,
    }
}

/// The two-character mark in front of a row.
pub fn mark_of(level: Level) -> &'static str {
    match level {
        Level::Good => "ok",
        Level::Note => "- ",
        Level::Attention => "! ",
        Level::Problem => "!!",
    }
}

/// How the chosen row of a list looks: filled where the person is working, and only underlined where something else has
/// the focus (the other pane, or a prompt that is open), so it can still be found.
pub fn chosen_row(focused: bool) -> Style {
    if focused {
        Style::new()
            .fg(Color::Black)
            .bg(ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }
}
