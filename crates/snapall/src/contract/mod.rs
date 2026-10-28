// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! snapall's invocation surface, declared once with argenv.
//!
//! This file is the single source of truth for flags, positionals and environment
//! variables: parsing, `--help`, the published `snapall contract` document and the
//! validation of invocations all derive from these declarations.

mod commands;
mod inputs;
#[cfg(test)]
mod tests;

pub use commands::*;
pub use inputs::*;

use commands::{DESTROY_COMMANDS, DRY_RUN_COMMANDS, JSON_COMMANDS, LOGGED_COMMANDS, YES_COMMANDS};
