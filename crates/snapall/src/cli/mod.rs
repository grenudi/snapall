// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Turning what was typed, and the environment, into a request that is known to be valid ([`Parsed`]).
//!
//! Everything here is a pure function of its inputs (no process state), so every rule is a
//! plain test. argenv resolves and lints; this module decides policy and parses the
//! survivors into domain types, once, at the boundary.

mod building;
mod parsing;
mod request;
#[cfg(test)]
mod tests;

pub use parsing::{commands, parse};
pub use request::{Logging, Parsed, Request, UsageError};

use building::build_request;
pub(crate) use building::parse_only;
use parsing::flag;
use request::usage_error;
