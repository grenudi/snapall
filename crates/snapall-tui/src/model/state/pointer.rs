// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The mouse: what it is over, and what was just clicked.

use crate::model::Click;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pointer {
    /// What the mouse is over, if it is something that can be clicked: it is lit, and a button says what it does
    /// on the footer line while it is.
    pub hover: Option<Click>,
    /// What was clicked a moment ago, and for how many more ticks it is shown pressed.
    pub pressed: Option<(Click, u8)>,
}
