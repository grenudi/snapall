// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Looking, which changes nothing: the areas, their snapshots, the history of a file, the health, the record, the restore points.

use super::*;
use crate::output::Outcome;
use crate::{advice, render};
use snapall_core::{
    all_snapshots, assess, file_history, gather, preview_restore, restore_points, snapshots_of,
    ConfigName, Level, Preview, SnapshotId,
};
use std::process::ExitCode;

pub fn list(ctx: &Context) -> Outcome {
    let configs = ctx.backend.list_configs()?;
    ctx.show
        .result(render::configs(&configs), render::configs_json(&configs))
}

pub fn snapshots(ctx: &Context, config: Option<ConfigName>) -> Outcome {
    let all = match config {
        Some(config) => {
            let snapshots = snapshots_of(ctx.backend, &config)?;
            vec![(config, snapshots)]
        }
        None => all_snapshots(ctx.backend)?,
    };
    ctx.show
        .result(render::snapshots(&all), render::snapshots_json(&all))
}

/// How one file stood in each snapshot.
pub fn history(ctx: &Context, path: &str) -> Outcome {
    let path = absolute(path)?;
    let found = file_history(ctx.backend, &path).map_err(|e| advice::explain(&e, &path))?;
    ctx.show
        .result(render::history(&found), render::history_json(&found))
}

/// What restoring a snapshot would change, without changing it.
pub fn changes(ctx: &Context, config: &ConfigName, id: SnapshotId, path: Option<&str>) -> Outcome {
    let preview = preview_restore(ctx.backend, config, id, path)?;
    show_changes(ctx, config, id, &preview)
}

/// How protection stands: whether it is working, the room it takes, whether old snapshots are cleaned
/// up, and what to do about anything that is not. It exits with failure only for what needs doing now.
pub fn health(ctx: &Context) -> Outcome {
    let facts = gather(ctx.backend, &snapall_system::LinuxSystem::new())?;
    let report = assess(&facts);
    ctx.show.result(
        render::health(&facts, &report),
        render::health_json(&facts, &report),
    )?;
    Ok(if report.verdict.level == Level::Problem {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// What snapall has done, newest first: from the window and from commands, in this and earlier sessions.
pub fn activity(ctx: &Context) -> Outcome {
    use snapall_core::SystemProbe;
    let entries = snapall_system::LinuxSystem::new().history(100);
    ctx.show
        .result(render::activity(&entries), render::activity_json(&entries))
}

/// The restore points: the snapshots that one action took together.
pub fn points(ctx: &Context) -> Outcome {
    let found = restore_points(&all_snapshots(ctx.backend)?);
    ctx.show
        .result(render::points(&found), render::points_json(&found))
}

pub(super) fn show_changes(
    ctx: &Context,
    config: &ConfigName,
    id: SnapshotId,
    preview: &Preview,
) -> Outcome {
    ctx.show.result(
        render::changes(config, id, preview),
        render::changes_json(config, id, preview),
    )
}
