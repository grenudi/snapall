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
    preview_restore, restore_snapshot, AreaName, Preview, RestoreError, SnapshotId,
};
use std::error::Error;

/// Put a config, or one path of it, back to how a snapshot had it. It shows what will change, asks
/// first (unless told not to), and the use case takes a snapshot of the current state before it
/// touches anything.
pub fn restore(
    ctx: &Context,
    area: &AreaName,
    id: SnapshotId,
    path: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Outcome {
    let preview = preview_restore(ctx.backend, area, id, path)?;
    refuse_to_restore_the_whole_system_root(&preview, area, path)?;
    if dry_run || preview.changes.is_empty() {
        return show_changes(ctx, area, id, &preview);
    }
    let approved = ask_before_restoring(yes, area, id, &preview)?;
    let (now, past) = match path {
        Some(path) => (
            format!("Restoring {path} from snapshot #{id} of {area}"),
            format!("Restored {path} from snapshot #{id} of {area}"),
        ),
        None => (
            format!("Putting {area} back to snapshot #{id}"),
            format!("Put {area} back to snapshot #{id}"),
        ),
    };
    let change = Change::begin(approved, &now, &past, None)?;
    let result = restore_snapshot(ctx.backend, area, id, path);
    if let Ok(done) = &result {
        change.rollback(snapall_core::rollback::after_restoring(
            area,
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
        render::restored(area, id, &done),
        render::restored_json(area, id, &done),
    )
}

/// Restoring all of the system root while it runs would leave files, kernel and boot menu out of
/// step: a deliberate refusal (not a usage error, not a crash), with the way that works instead.
pub(super) fn refuse_to_restore_the_whole_system_root(
    preview: &Preview,
    area: &AreaName,
    path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if preview.is_system_root && path.is_none() {
        let route = boot::restore_route(&boot::assess(&boot_probe::system()));
        let refusal = RestoreError::SystemRoot { area: area.clone() };
        return Err(format!("{refusal}\n{route}").into());
    }
    Ok(())
}

fn restore_question(preview: &Preview, area: &AreaName, id: SnapshotId) -> String {
    let count = preview.changes.len();
    let paths = if count == 1 { "path" } else { "paths" };
    format!(
        "Restore {count} {paths} of {area} to #{id}? The current state is snapshotted first, so this can be undone."
    )
}

fn ask_before_restoring(
    yes: bool,
    area: &AreaName,
    id: SnapshotId,
    preview: &Preview,
) -> Result<ask::Approved, Box<dyn std::error::Error>> {
    ask::before_showing(
        Act::Restore,
        yes,
        || {
            format!(
                "{}\n{}\n",
                render::changes(area, id, preview),
                snapall_core::explain::RESTORE_CAUTION
            )
        },
        &restore_question(preview, area, id),
    )
}
