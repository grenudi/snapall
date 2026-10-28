// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support: an in-memory adapter and the contract every backend must satisfy.
//! Compiled for this crate's own tests and for adapter crates via the `testing` feature.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod backend;
mod building;
mod contract;
mod state;
mod system;
#[cfg(test)]
mod tests;

pub use contract::backend_contract;
pub use state::InMemoryBackend;
pub use system::FakeSystem;

use state::{AreaState, State};
