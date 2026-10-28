// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The in-memory stand-in and the contract, tested against themselves.

#![cfg(test)]

use crate::domain::AreaName;

use super::*;

#[test]
fn in_memory_backend_satisfies_the_contract() {
    let backend = InMemoryBackend::with_areas(&["root"]);
    backend_contract(&backend, &AreaName::parse("root").unwrap());
}
