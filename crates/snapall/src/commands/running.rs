// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Changes that are running: asking one to stop.

use super::*;
use crate::ask;
use crate::output::Outcome;
use snapall_core::Act;

/// Ask a running change to stop after the area it is on. Only a request: the change looks for it between
/// its areas, never inside one, so nothing is left half-done.
pub fn cancel(ctx: &Context, job: &str, yes: bool) -> Outcome {
    use snapall_core::{JournalStatus, SystemProbe};
    let system = snapall_system::LinuxSystem::new();
    let history = system.history(500);
    let Some(entry) = history.iter().find(|entry| entry.id == job) else {
        return Err(format!(
            "there is no change called `{job}`: `snapall activity --json` lists them"
        )
        .into());
    };
    if entry.status != JournalStatus::Running {
        return Err(format!(
            "`{}` has already ended ({})",
            entry.what,
            entry.status.word().to_lowercase()
        )
        .into());
    }
    let question = format!("Ask {} to stop after the area it is on?", entry.what);
    ask::gate(Act::StopChange.weight(), yes, &question, None)?;
    system.request_stop(job)?;
    ctx.show.result(
        format!(
            "Asked {} to stop. It finishes the area it is on, then stops; nothing is left half-done.\n",
            entry.what
        ),
        serde_json::json!({ "asked": job, "what": entry.what }),
    )
}
