// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running one of the steps that install what is missing, as the command it is.

use snapall_core::protecting::doctor::Step;
use std::process::Command;

/// Run the step; what went wrong, if anything, in words.
pub fn run_step(step: &Step) -> Result<(), String> {
    match Command::new(step.program).args(&step.args).status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("exited with {status}")),
        Err(error) => Err(format!("could not run `{}`: {error}", step.program)),
    }
}
