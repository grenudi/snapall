// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Health: is protection working, how much room does it take, will old snapshots be cleaned up.
//!
//! Facts go in, a report comes out, and nothing here reads the machine. The story is `assess`, below:
//! each question is one small rule that says what it found, why it matters and, where snapall can do
//! something about it, what. The words are plain ones (see ADR 16); a figure that cannot be measured is
//! said to be unmeasured and why, never guessed.

mod chips;
mod facts;
mod findings;
mod sizes;
#[cfg(test)]
mod tests;
mod verdicts;

pub use facts::{gather, AreaFacts, Facts, JobFacts};
pub use findings::assess;
pub use sizes::format_bytes;
pub use verdicts::{Chip, Finding, Health, Level, Remedy, Topic, Verdict};

use chips::chips;
use findings::{cleanup_state, fullest_disk};
