// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a change is called in the Activity tab, and how it went: the words, apart from the doing.

mod background;
mod describing;
mod outcome;
#[cfg(test)]
mod tests;

pub(crate) use background::{background_arguments, new_id};
pub(crate) use describing::{describe_change, describe_now};
pub(crate) use outcome::{how_it_went, outcome_of, way_back};
