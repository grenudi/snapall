// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `snapall doctor`, as pure functions: from facts about the machine to findings and an
//! install plan. Nothing here reads a file, runs a program or asks who the user is; the
//! shell probes the machine and hands the facts in, so every distribution is a unit test.

mod diagnosing;
mod installing;
#[cfg(test)]
mod tests;

pub use diagnosing::{diagnose, worst, Check, Probe, Status};
pub use installing::{
    describe, family, install_plan, Family, InstallPlan, OsRelease, Step, PACKAGE,
};
