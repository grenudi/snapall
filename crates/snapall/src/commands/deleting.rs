// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deleting a snapshot: it says which one, and asks for `destroy`.

use super::*;
use crate::output::Outcome;
use crate::{ask, render};
use snapall_core::{delete_snapshot, Act, ConfigName, SnapshotId};

pub fn delete(ctx: &Context, config: &ConfigName, id: SnapshotId, destroy: bool) -> Outcome {
    // What is about to be lost is said first. A snapshot that is not there is not asked about: the attempt
    // fails and says so, and the failed attempt is in the audit trail, as it always was.
    if let Some(snapshot) = ctx
        .backend
        .list_snapshots(config)?
        .into_iter()
        .find(|snapshot| snapshot.id == id)
    {
        let what = format!(
            "snapshot #{id} of {config}{}{}",
            snapshot
                .date
                .as_deref()
                .map_or(String::new(), |date| format!(" from {date}")),
            if snapshot.description.is_empty() {
                String::new()
            } else {
                format!(" (\"{}\")", snapshot.description)
            }
        );
        eprintln!("{what} will be deleted for good. Only this saved moment goes: the files in {config} are not touched.\n");
        if let Some(stop) = ask::gate(
            Act::DeleteSnapshots.weight(),
            destroy,
            &format!("Delete {what}?"),
            None,
        )? {
            return Ok(stop);
        }
    }
    let change = Change::begin(
        &format!("Deleting 1 snapshot of {config}"),
        &format!("Deleted 1 snapshot of {config}"),
        None,
    )?;
    let result = delete_snapshot(ctx.backend, config, id);
    if result.is_ok() {
        change.rollback(snapall_core::Rollback::Advice(
            snapall_core::explain::DELETE_ADVICE.to_owned(),
        ));
    }
    change.end(
        result
            .as_ref()
            .map(|()| String::new())
            .map_err(ToString::to_string),
    );
    result?;
    ctx.show.result(
        render::deleted(config, id),
        render::deleted_json(config, id),
    )
}
