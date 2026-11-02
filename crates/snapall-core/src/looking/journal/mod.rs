// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall did, remembered: every change writes an event when it starts and another when it ends,
//! whichever snapall made it (the window or a command), and folding them gives the record a person reads.
//!
//! Two events instead of one, because a change that has started and never ends is itself a fact: it is
//! still running, or its process is gone (the computer was switched off, the terminal killed) and it was
//! *interrupted*, which a record written only at the end could never say.

mod events;
mod history;
#[cfg(test)]
mod tests;

pub use events::{Event, Outcome, Rollback, Source, Undo};
pub use history::{fold, Entry, Progress, Status};
