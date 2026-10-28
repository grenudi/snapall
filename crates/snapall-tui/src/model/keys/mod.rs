// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What each key and click does, and how a prompt answers one. A key means what the screen in front of it means.

mod answering;
mod clicking;
mod cursor;
mod screen;

pub(crate) use answering::{on_modal_key, Reaction};
pub(crate) use clicking::on_click;
pub(crate) use cursor::{
    clamp_step, jump, load_selected_if_needed, on_scroll, select_area, step, toggle_mark,
};
pub(crate) use screen::on_key;
