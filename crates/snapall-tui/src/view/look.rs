// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How things that can be clicked look: at rest, under the mouse, and just clicked. One place, so that the
//! button bar, the tabs and the prompts' buttons are recognisably the same kind of thing.
//!
//! The key is a bright chip and the label a block of colour beside it, like the function-key bar of
//! `htop` and Midnight Commander: a row of these is already read as buttons by anyone who uses a terminal.
//! Only the 16 named colours are used, so it looks right in any terminal and follows its colour scheme.

use crate::model::Click;
use crate::view::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mood {
    Rest,
    /// The mouse is over it.
    Hover,
    /// It was clicked a moment ago.
    Pressed,
    /// It does nothing where it is: it is drawn dim and does not answer the mouse.
    Disabled,
}

pub(crate) fn mood(state: &State, click: Click) -> Mood {
    if state.pressed.is_some_and(|(pressed, _)| pressed == click) {
        Mood::Pressed
    } else if state.hover == Some(click) {
        Mood::Hover
    } else {
        Mood::Rest
    }
}

/// The key letter of a button: a bright chip, red for what removes something.
pub(crate) fn chip(mood: Mood, danger: bool) -> Style {
    let base = Style::new().bold();
    match (mood, danger) {
        (Mood::Disabled, _) => Style::new().fg(Color::DarkGray),
        (Mood::Pressed, _) => base.fg(Color::Black).bg(Color::Green),
        (Mood::Hover, true) => base.fg(Color::White).bg(Color::LightRed),
        (Mood::Hover, false) => base.fg(Color::Black).bg(Color::White),
        (Mood::Rest, true) => base.fg(Color::White).bg(Color::Red),
        (Mood::Rest, false) => base.fg(Color::Black).bg(Color::Yellow),
    }
}

/// The label of a button: a block of colour that brightens under the mouse and flashes when clicked.
pub(crate) fn label(mood: Mood, danger: bool) -> Style {
    match (mood, danger) {
        (Mood::Disabled, _) => Style::new().fg(Color::DarkGray),
        (Mood::Pressed, _) => Style::new().fg(Color::Black).bg(Color::Green).bold(),
        (Mood::Hover, true) => Style::new().fg(Color::White).bg(Color::Red),
        (Mood::Hover, false) => Style::new().fg(Color::White).bg(Color::Blue),
        (Mood::Rest, _) => Style::new().fg(Color::White).bg(Color::DarkGray),
    }
}

/// A tab: the one showing is a white block, the others are buttons like the rest.
pub(crate) fn tab(mood: Mood, showing: bool) -> Style {
    match (mood, showing) {
        (Mood::Disabled, _) => Style::new().fg(Color::DarkGray),
        (Mood::Pressed, _) => Style::new().fg(Color::Black).bg(Color::Green).bold(),
        (_, true) => Style::new().fg(Color::Black).bg(Color::White).bold(),
        (Mood::Hover, false) => Style::new().fg(Color::White).bg(Color::Blue),
        (Mood::Rest, false) => Style::new().fg(Color::Gray).bg(Color::DarkGray),
    }
}

/// A button inside a prompt: the answer it asks for is in the prompt's own colour, the way out is grey.
pub(crate) fn prompt_button(mood: Mood, answers: bool, colour: Color) -> Style {
    match (mood, answers) {
        (Mood::Disabled, _) => Style::new().fg(Color::DarkGray),
        (Mood::Pressed, _) => Style::new().fg(Color::Black).bg(Color::Green).bold(),
        (Mood::Hover, true) => Style::new().fg(Color::Black).bg(Color::White).bold(),
        (Mood::Rest, true) => Style::new().fg(Color::Black).bg(colour).bold(),
        (Mood::Hover, false) => Style::new().fg(Color::White).bg(Color::Blue),
        (Mood::Rest, false) => Style::new().fg(Color::White).bg(Color::DarkGray),
    }
}
