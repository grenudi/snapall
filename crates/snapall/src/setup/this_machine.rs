// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Setup on this machine: its facts read from it, and a terminal or a silence to talk through.

use super::*;
use crate::output::emit;
use snapall_core::keeping::jobs::JobAction;
use snapall_core::vocabulary::explain::{self};
use snapall_core::SystemProbe;
use std::error::Error;

/// Read the facts of this machine.
pub fn gather() -> Facts {
    let machine = snapall_system::LinuxSystem::new();
    let cleanup_job = machine.service(snapall_core::keeping::jobs::CLEANUP.unit);
    Facts {
        mountinfo: snapall_system::environment::mounts().unwrap_or_default(),
        is_root: snapall_system::environment::is_administrator() == Some(true),
        cleanup: explain::cleanup_from(
            cleanup_job.enabled.as_deref(),
            cleanup_job.active.as_deref(),
        ),
        cleanup_schedule: machine
            .unit_details(snapall_core::keeping::jobs::CLEANUP.unit)
            .map(|details| details.schedule())
            .unwrap_or_default(),
    }
}

/// A terminal: it says things on stdout and asks on stderr.
pub struct Terminal;

impl Talk for Terminal {
    fn say(&mut self, text: &str) {
        let _ = emit(&format!("{text}\n"));
    }

    fn ask(&mut self, question: &str) -> Result<bool, Box<dyn Error>> {
        if !crate::ask::can_ask() {
            return Err(
                "snapall needs a terminal to ask you; add --yes to accept what it recommends"
                    .into(),
            );
        }
        // typed, like every change: Enter alone is not a yes
        crate::ask::yes_no(question)
    }

    fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>> {
        let machine = snapall_system::LinuxSystem::new();
        machine
            .control_job(
                snapall_core::keeping::jobs::CLEANUP.unit,
                JobAction::EnableNow,
            )
            .map_err(Into::into)
    }
}

/// For `--json`: nothing is said, and nothing is asked.
pub struct Quiet;

impl Talk for Quiet {
    fn say(&mut self, _text: &str) {}

    fn ask(&mut self, _question: &str) -> Result<bool, Box<dyn Error>> {
        Ok(false)
    }

    fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>> {
        Terminal.switch_on_cleanup()
    }
}
