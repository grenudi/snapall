// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting one area, or one file of it, back to how a snapshot had it.

use super::*;
use crate::output::Outcome;
use crate::{ask, boot_probe, render};
use snapall_core::boot;
use snapall_core::Act;
use snapall_core::{
    preview_restore, restore_snapshot, ConfigName, Preview, RestoreError, SnapshotId,
};
use std::error::Error;

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
    let approved = ask_before_restoring(yes, config, id, &preview)?;
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
    let change = Change::begin(approved, &now, &past, None)?;
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

fn restore_question(preview: &Preview, config: &ConfigName, id: SnapshotId) -> String {
    let count = preview.changes.len();
    let paths = if count == 1 { "path" } else { "paths" };
    format!(
        "Restore {count} {paths} of {config} to #{id}? The current state is snapshotted first, so this can be undone."
    )
}

fn ask_before_restoring(
    yes: bool,
    config: &ConfigName,
    id: SnapshotId,
    preview: &Preview,
) -> Result<ask::Approved, Box<dyn std::error::Error>> {
    ask::before_showing(
        Act::Restore,
        yes,
        || {
            format!(
                "{}\n{}\n",
                render::changes(config, id, preview),
                snapall_core::explain::RESTORE_CAUTION
            )
        },
        &restore_question(preview, config, id),
    )
}
