// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rolling a change back to the state it recorded.

use super::*;
use crate::ask;
use crate::output::Outcome;
use snapall_core::{all_snapshots, restore_points, Act, ConfigName};
use std::error::Error;
use std::process::ExitCode;

/// Whether what a rollback would go back to is still there. It is checked when the rollback is asked for, not
/// when it was recorded: snapper's cleanup may have removed the snapshot since, and then the honest answer is
/// that it cannot be done.
pub(super) fn check_the_way_back_still_exists(
    ctx: &Context,
    commands: &[Vec<String>],
) -> Result<(), Box<dyn Error>> {
    for command in commands {
        match command
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["restore", config, id, ..] => {
                let config = ConfigName::parse(config)?;
                let number: u64 = id
                    .parse()
                    .map_err(|_| format!("`{id}` is not a snapshot number"))?;
                let there = ctx
                    .backend
                    .list_snapshots(&config)?
                    .iter()
                    .any(|snapshot| snapshot.id.get() == number);
                if !there {
                    return Err(format!(
                        "snapshot #{number} of {config}, which this would go back to, no longer exists: cleanup has removed it, so this cannot be rolled back"
                    )
                    .into());
                }
            }
            ["restore-all", point, ..] => {
                let there = restore_points(&all_snapshots(ctx.backend)?)
                    .iter()
                    .any(|candidate| candidate.point.to_string() == *point);
                if !there {
                    return Err(format!(
                        "restore point {point}, which this would go back to, no longer exists: cleanup has removed it, so this cannot be rolled back"
                    )
                    .into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Roll a change back to the state it recorded, if it recorded one. It says what that state is and what it
/// will run, asks as heavily as what it does weighs (undoing the taking of a snapshot is deleting it), and
/// runs the recorded commands through this same program, so they are checked and journaled like any other.
pub fn undo(ctx: &Context, change_id: &str, yes: bool, destroy: bool, no_dbus: bool) -> Outcome {
    use snapall_core::{Rollback, SystemProbe, Weight};
    let system = snapall_system::LinuxSystem::new();
    let history = system.history(500);
    let Some(entry) = history.iter().find(|entry| entry.id == change_id) else {
        return Err(format!(
            "there is no change called `{change_id}`: `snapall activity` lists them"
        )
        .into());
    };
    let undo = match &entry.rollback {
        Some(Rollback::Undo(undo)) => undo,
        Some(Rollback::Advice(text)) => {
            eprintln!("{} cannot be undone.\n{text}", entry.what);
            return Ok(ExitCode::FAILURE);
        }
        None => {
            return Err(format!(
                "no way back was recorded for `{}`: changes made before snapall recorded one have none",
                entry.what
            )
            .into());
        }
    };
    check_the_way_back_still_exists(ctx, &undo.commands)?;
    eprintln!("To roll back: {}\n  {}\n", entry.what, undo.what);
    for command in &undo.commands {
        eprintln!("  snapall {}", command.join(" "));
    }
    eprintln!();
    let said_so = if undo.weight == Weight::Destroys {
        destroy
    } else {
        yes
    };
    let act = Act::Undo { doing: undo.weight };
    ask::gate(act.weight(), said_so, "Roll this back?", None)?;
    let program = std::env::current_exe()?;
    for command in &undo.commands {
        let mut step = std::process::Command::new(&program);
        if no_dbus {
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
    ctx.show.result(
        format!("Rolled back: {}\n", undo.what),
        serde_json::json!({ "change": change_id, "rolled_back": true }),
    )
}
