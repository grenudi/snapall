// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `snapall setup`: find the parts of the disk that snapshots do not cover, explain what that means
//! in plain words, ask, and set them up. A person should never need a command outside snapall to
//! get protected, nor know a word snapall has not explained.
//!
//! The flow takes its facts and the way it talks and asks as inputs, so every branch is tested as a
//! conversation with a scripted person.

mod flow;
mod inputs;
mod offers;
mod report;
#[cfg(test)]
mod tests;
mod this_machine;

pub use flow::run;
pub use inputs::{Facts, Options, Talk};
pub use offers::{hint_for, offers};
pub use report::{AreaReport, Report};
pub use this_machine::{gather, Quiet, Terminal};

use flow::find_the_areas;
