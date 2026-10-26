// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a person does: a key, a click, a button. Independent of any terminal library.

use crate::model::*;

/// A key, independent of any terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    Enter,
    Esc,
    Backspace,
    CtrlC,
    Char(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Configs,
    Snapshots,
}

/// Something that can be clicked. Each is the mouse spelling of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Create,
    CreateAll,
    Delete,
    Restore,
    Reload,
    Help,
    Quit,
    /// The affirmative button of a prompt (Enter).
    Confirm,
    /// The negative button of a prompt (Esc).
    Cancel,
    /// See what has no snapshots yet and protect it.
    Protect,
    /// In a prompt that shows several things one at a time: the next one.
    Next,
    /// In the restore prompt: put every area of the snapshot's restore point back.
    RestoreAll,
    /// See the restore points and put every area back to one of them.
    Points,
}

impl Button {
    /// The key this button stands for, so a click goes through exactly the code the key does.
    pub fn key(self) -> Key {
        match self {
            Button::Create => Key::Char('c'),
            Button::CreateAll => Key::Char('a'),
            Button::Delete => Key::Char('d'),
            Button::Restore => Key::Char('u'),
            Button::Reload => Key::Char('r'),
            Button::Help => Key::Char('?'),
            Button::Quit => Key::Char('q'),
            Button::Confirm => Key::Enter,
            Button::Cancel => Key::Esc,
            Button::Protect => Key::Char('p'),
            Button::Next => Key::Tab,
            Button::RestoreAll => Key::Char('A'),
            Button::Points => Key::Char('R'),
        }
    }
}

/// A left click, already resolved by the view to what is under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Button(Button),
    /// A row of a pane (its position in what is displayed, not on screen).
    Row {
        pane: Pane,
        index: usize,
    },
    /// The mark column of a snapshot row: toggles the mark without moving the cursor.
    Mark {
        index: usize,
    },
    /// A name in the tab bar.
    Tab(Tab),
    /// An item of the status bar: goes to the Health tab, at the row that explains it.
    Chip(snapall_core::Topic),
    /// A row of the Health or Activity tab, whichever is showing.
    TabRow {
        index: usize,
    },
}
