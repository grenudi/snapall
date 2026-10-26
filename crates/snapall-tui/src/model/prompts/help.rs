// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The help: two pages, turned with Enter, Tab or the arrows.

use crate::model::*;

/// The help: Enter, Tab or the arrows turn the page, Esc or `q` close it.
pub(crate) fn reading_help(key: Key, page: usize) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Enter | Key::Tab | Key::Right | Key::Left | Key::Char(' ' | 'h' | 'l') => {
            Reaction::stays(Modal::Help {
                page: (page + 1) % HELP_PAGES,
            })
        }
        _ => Reaction::stays(Modal::Help { page }),
    }
}

/// How many pages the help has.
pub const HELP_PAGES: usize = 2;
