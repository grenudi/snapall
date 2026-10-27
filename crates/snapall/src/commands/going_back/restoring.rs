// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting one area, or one file of it, back to how a snapshot had it.

use super::*;
use crate::output::Outcome;
use crate::{ask, boot_probe, render};
use snapall_core::boot;
use snapall_core::{
    preview_restore, restore_snapshot, ConfigName, Preview, RestoreError, SnapshotId,
};
use std::error::Error;
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
