// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Going back: putting an area or a restore point back, and rolling a change back with `undo`.

use super::*;
use crate::output::Outcome;
use crate::{ask, boot_probe, render};
use snapall_core::boot;
use snapall_core::{
    all_snapshots, fresh_point, preview_point, preview_restore, restore_point_along,
    restore_points, restore_snapshot, Act, ConfigName, MemberView, Point, Preview, RestoreError,
    RestorePoint, SnapshotId,
};
use std::error::Error;
use std::io::IsTerminal;
use std::process::ExitCode;

/// Put a config, or one path of it, back to how a snapshot had it. It shows what will change, asks
/// first (unless told not to), and the use case takes a snapshot of the current state before it
/// touches anything.
pub fn restore(
    ctx: &Context,
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Outcome {
    let preview = preview_restore(ctx.backend, config, id, path)?;
    refuse_to_restore_the_whole_system_root(&preview, config, path)?;
    if dry_run || preview.changes.is_empty() {
        return show_changes(ctx, config, id, &preview);
    }
    match confirmation(yes, config, id, &preview)? {
        Confirmation::Given => {}
        Confirmation::NoTerminalToAskOn => return Ok(ExitCode::from(2)),
        Confirmation::Declined => return Ok(ExitCode::FAILURE),
    }
    let (now, past) = match path {
        Some(path) => (
            format!("Restoring {path} from snapshot #{id} of {config}"),
            format!("Restored {path} from snapshot #{id} of {config}"),
        ),
        None => (
            format!("Putting {config} back to snapshot #{id}"),
            format!("Put {config} back to snapshot #{id}"),
        ),
    };
    let change = Change::begin(&now, &past, None)?;
    let result = restore_snapshot(ctx.backend, config, id, path);
    if let Ok(done) = &result {
        change.rollback(snapall_core::rollback::after_restoring(
            config,
            path,
            done.safety_snapshot,
        ));
    }
    change.end(
        result
            .as_ref()
            .map(|_| String::new())
            .map_err(ToString::to_string),
    );
    let done = result?;
    ctx.show.result(
        render::restored(config, id, &done),
        render::restored_json(config, id, &done),
    )
}

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
    if let Some(stop) = ask::gate(act.weight(), said_so, "Roll this back?", None)? {
        return Ok(stop);
    }
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

/// Put every area back to one restore point with a single action: shows what would change in each,
/// asks first (unless told not to), takes a safety snapshot per area (all one restore point, so
/// that undoing this is one action too) and reports each area. The running system is left alone.
pub fn restore_all(
    ctx: &Context,
    wanted: &Point,
    dry_run: bool,
    yes: bool,
    job: Option<&str>,
) -> Outcome {
    let found = restore_points(&all_snapshots(ctx.backend)?);
    let point = the_point_named(&found, wanted)?;
    let previews = preview_point(ctx.backend, point);
    let route = boot::restore_route(&boot::assess(&boot_probe::system()));
    let anything_to_change = previews
        .iter()
        .any(|p| matches!(&p.view, MemberView::Changes(c) if !c.changes.is_empty()));
    if dry_run || !anything_to_change {
        return ctx.show.result(
            render::point_preview(point, &previews, &route),
            render::point_preview_json(point, &previews),
        );
    }
    match point_confirmation(yes, point, &previews, &route) {
        Confirmation::Given => {}
        Confirmation::NoTerminalToAskOn => return Ok(ExitCode::from(2)),
        Confirmation::Declined => return Ok(ExitCode::FAILURE),
    }
    let change = Change::begin(
        &format!("Putting every area back to restore point {}", point.point),
        &format!("Put every area back to restore point {}", point.point),
        job,
    )?;
    let done = restore_point_along(
        ctx.backend,
        point,
        fresh_point(ctx.backend, unix_now()),
        &change.course(),
    );
    if !done.members.is_empty() {
        change.rollback(snapall_core::rollback::after_restoring_all(
            done.safety.as_str(),
        ));
    }
    if done.stopped {
        let (put_back, total) = (done.members.len(), point.members.len());
        eprintln!("snapall: stopped on request after {put_back} of {total} areas; the rest were left as they are");
        change.end_stopped(format!(
            "stopped on request after {put_back} of {total} areas: the others were left as they are. To undo what was done: restore point {}",
            done.safety.as_str()
        ));
        ctx.show.result(
            render::point_restored(point, &done),
            render::point_restored_json(point, &done),
        )?;
        return Ok(ExitCode::FAILURE);
    }
    change.end(if done.succeeded() {
        let put_back = done
            .members
            .iter()
            .filter(|m| matches!(m.result, snapall_core::MemberResult::Restored(_)))
            .count();
        Ok(if done.changed_anything() {
            format!(
                "{put_back} area{} put back. To undo it: restore point {}",
                if put_back == 1 { "" } else { "s" },
                done.safety.as_str()
            )
        } else {
            String::new()
        })
    } else {
        Err("some areas could not be put back: the output says which".to_owned())
    });
    ctx.show.result(
        render::point_restored(point, &done),
        render::point_restored_json(point, &done),
    )?;
    Ok(if done.succeeded() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub(super) fn the_point_named<'a>(
    found: &'a [RestorePoint],
    wanted: &Point,
) -> Result<&'a RestorePoint, Box<dyn Error>> {
    found.iter().find(|p| p.point == *wanted).ok_or_else(|| {
        if found.is_empty() {
            "There are no restore points yet. `sudo snapall` takes a snapshot of every area at once, and that is one.".into()
        } else {
            format!("`{wanted}` is not a restore point. `snapall points` lists them.").into()
        }
    })
}

/// Putting areas back changes files, so the person is asked, unless they said `--yes`.
pub(super) fn point_confirmation(
    yes: bool,
    point: &RestorePoint,
    previews: &[snapall_core::MemberPreview],
    route: &str,
) -> Confirmation {
    if yes {
        return Confirmation::Given;
    }
    if !ask::can_ask() {
        eprintln!(
            "snapall: restore-all changes files and there is no terminal to ask on; \
             look first with --dry-run, then pass --yes"
        );
        return Confirmation::NoTerminalToAskOn;
    }
    eprint!("{}", render::point_preview(point, previews, route));
    eprintln!("\n{}\n", snapall_core::explain::RESTORE_CAUTION);
    let areas = previews
        .iter()
        .filter(|p| matches!(&p.view, MemberView::Changes(c) if !c.changes.is_empty()))
        .count();
    let question = format!(
        "Put {areas} area{} back to restore point {}? Each one's current state is snapshotted first, so this can be undone.",
        if areas == 1 { "" } else { "s" },
        point.point
    );
    match ask::yes_no(&question) {
        Ok(true) => Confirmation::Given,
        _ => {
            eprintln!("snapall: not restoring");
            Confirmation::Declined
        }
    }
}

/// Restoring all of the system root while it runs would leave files, kernel and boot menu out of
/// step: a deliberate refusal (not a usage error, not a crash), with the way that works instead.
pub(super) fn refuse_to_restore_the_whole_system_root(
    preview: &Preview,
    config: &ConfigName,
    path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if preview.is_system_root && path.is_none() {
        let route = boot::restore_route(&boot::assess(&boot_probe::system()));
        let refusal = RestoreError::SystemRoot {
            config: config.clone(),
        };
        return Err(format!("{refusal}\n{route}").into());
    }
    Ok(())
}

pub(super) enum Confirmation {
    Given,
    /// Nobody to ask: the caller must say `--yes`, so nothing is changed.
    NoTerminalToAskOn,
    Declined,
}

/// Restoring changes files, so the person is asked, unless they said `--yes`.
pub(super) fn confirmation(
    yes: bool,
    config: &ConfigName,
    id: SnapshotId,
    preview: &Preview,
) -> Result<Confirmation, Box<dyn Error>> {
    if yes {
        return Ok(Confirmation::Given);
    }
    if !ask::can_ask() {
        eprintln!(
            "snapall: restore changes files and there is no terminal to ask on; \
             look first with `snapall changes`, then pass --yes"
        );
        return Ok(Confirmation::NoTerminalToAskOn);
    }
    eprint!("{}", render::changes(config, id, preview));
    eprintln!("\n{}\n", snapall_core::explain::RESTORE_CAUTION);
    let count = preview.changes.len();
    let paths = if count == 1 { "path" } else { "paths" };
    let question = format!(
        "Restore {count} {paths} of {config} to #{id}? The current state is snapshotted first, so this can be undone."
    );
    if ask::yes_no(&question)? {
        Ok(Confirmation::Given)
    } else {
        eprintln!("snapall: not restoring");
        Ok(Confirmation::Declined)
    }
}

/// After a snapshot: a nudge to a person, on stderr (stdout stays parseable), and only when a
/// boot-menu tool would help.
pub(super) fn suggest_the_boot_menu(ctx: &Context) {
    let to_a_person = std::io::stderr().is_terminal() && !ctx.show.is_json();
    if !ctx.hints || !to_a_person {
        return;
    }
    if let Some(tip) = boot::hint(&boot::assess(&boot_probe::system())) {
        eprintln!("{tip}");
    }
}
