// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The snapshot backend for snapper: our verbs said to it, and what it says read back in our words.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod backend;
mod files;
mod reading;
mod running;
#[cfg(test)]
mod tests;

pub use backend::SnapperCli;
pub use running::{CommandOutput, CommandRunner, SystemRunner};
