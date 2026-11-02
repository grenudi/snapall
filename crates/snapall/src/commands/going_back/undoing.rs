// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rolling a change back to the state it recorded.

use super::*;
use crate::ask;
use crate::output;
use crate::output::Outcome;
use snapall_core::Act;
use std::error::Error;
use std::process::ExitCode;

pub fn undo(ctx: &Context, change_id: &str, agreed: Agreed) -> Outcome {
    use snapall_core::SystemProbe;
    let history = snapall_system::LinuxSystem::new().history(500);
    let entry = the_change_called(&history, change_id)?;
    let Some(undo) = the_way_back(entry)? else {
        return Ok(ExitCode::FAILURE);
    };
    snapall_core::going_back::still_there::still_there(ctx.backend, undo)?;
    say_what_it_would_run(entry, undo);
    let act = Act::Undo { doing: undo.weight };
    let _asked = ask::gate(act, agreed, "Roll this back?", None)?;
    run_the_way_back(ctx, undo)?;
    ctx.show.result(
        format!("Rolled back: {}\n", undo.what),
        serde_json::json!({ "change": change_id, "rolled_back": true }),
    )
}

fn the_change_called<'a>(
    history: &'a [snapall_core::JournalEntry],
    change_id: &str,
) -> Result<&'a snapall_core::JournalEntry, Box<dyn Error>> {
    history
        .iter()
        .find(|entry| entry.id == change_id)
        .ok_or_else(|| {
            format!("there is no change called `{change_id}`: `snapall activity` lists them").into()
        })
}

/// What undoes it. `None` is a change that cannot be undone, which has been said.
fn the_way_back(
    entry: &snapall_core::JournalEntry,
) -> Result<Option<&snapall_core::Undo>, Box<dyn Error>> {
    use snapall_core::Rollback;
    match &entry.rollback {
        Some(Rollback::Undo(undo)) => Ok(Some(undo)),
        Some(Rollback::Advice(text)) => {
            output::note(&format!("{} cannot be undone.\n{text}", entry.what));
            Ok(None)
        }
        None => Err(format!(
            "no way back was recorded for `{}`: changes made before snapall recorded one have none",
            entry.what
        )
        .into()),
    }
}

fn say_what_it_would_run(entry: &snapall_core::JournalEntry, undo: &snapall_core::Undo) {
    output::note(&format!("To roll back: {}\n  {}\n", entry.what, undo.what));
    for command in &undo.commands {
        output::note(&format!("  snapall {}", command.join(" ")));
    }
    output::note("");
}

/// Each command, one after another, by this same program; one failing stops the rest.
fn run_the_way_back(ctx: &Context, undo: &snapall_core::Undo) -> Result<(), Box<dyn Error>> {
    let program = std::env::current_exe()?;
    for command in &undo.commands {
        let mut step = std::process::Command::new(&program);
        if ctx.no_dbus {
            step.arg("--no-dbus");
        }
        step.args(command);
        // it has been answered here, at the weight of what it does
        if let Some(flag) = undo.weight.script_flag() {
            step.arg(flag);
        }
        if !step.status()?.success() {
            return Err(format!(
                "`snapall {}` did not succeed, so the rollback stopped there and nothing after it was done",
                command.join(" ")
            )
            .into());
        }
    }
    Ok(())
}
