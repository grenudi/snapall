// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A terminal UI for snapall: a second *driving adapter* over the same port as the CLI.
//!
//! Structured as the canon's functional core and imperative shell:
//!
//! * [`model`]: all behaviour. `update(state, msg) -> effects` is a pure function with no
//!   terminal, no clock and no I/O, so every key sequence is a unit test.
//! * [`view`]: `render(state, frame)` only draws.
//! * [`session`]: the shell that runs effects against a `SnapshotBackend` and feeds the
//!   results back as messages.
//! * [`term`]: the terminal, key translation and the event loop.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod model;
pub mod session;
pub mod term;
pub mod view;

pub use model::{FileTarget, UiContext};
pub use session::{Executor, Session};
pub use term::{run, Exit};
