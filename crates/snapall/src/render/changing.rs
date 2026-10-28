// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a restore would change, what a restore or a deletion did.

use super::*;
use serde_json::{json, Value};
use snapall_core::{AreaName, Preview, Restored, SnapshotId};

/// How many changed paths a human sees before the rest is summarised. `--json` is never cut.
pub(super) const SHOWN: usize = 200;

pub fn changes(config: &AreaName, id: SnapshotId, preview: &Preview) -> String {
    if preview.changes.is_empty() {
        return format!("{config} already matches #{id}: nothing would change\n");
    }
    let count = preview.changes.len();
    let mut out = format!(
        "{config}: restoring #{id} would change {count} path{}:\n",
        if count == 1 { "" } else { "s" }
    );
    for change in preview.changes.iter().take(SHOWN) {
        out.push_str(&format!("  {:<10} {}\n", verb(change.kind), change.path));
    }
    if count > SHOWN {
        out.push_str(&format!(
            "  ... and {} more (use --json for all of them)\n",
            count - SHOWN
        ));
    }
    out
}

pub fn changes_json(config: &AreaName, id: SnapshotId, preview: &Preview) -> Value {
    let changes: Vec<Value> = preview
        .changes
        .iter()
        .map(|c| json!({"kind": kind_word(c.kind), "path": c.path}))
        .collect();
    json!({
        "config": config.as_str(),
        "snapshot": id.get(),
        "subvolume": preview.location,
        "system_root": preview.is_system_root,
        "changes": changes,
    })
}

pub fn restored(config: &AreaName, id: SnapshotId, done: &Restored) -> String {
    let s = done.summary;
    format!(
        "restored {config} to #{id}: {} reverted, {} removed, {} brought back\n\
         undo this with: snapall restore {config} {}  (the state before is snapshot #{})\n",
        s.modified, s.deleted, s.created, done.safety_snapshot, done.safety_snapshot
    )
}

pub fn restored_json(config: &AreaName, id: SnapshotId, done: &Restored) -> Value {
    json!({
        "config": config.as_str(),
        "snapshot": id.get(),
        "safety_snapshot": done.safety_snapshot.get(),
        "reverted": done.summary.modified,
        "removed": done.summary.deleted,
        "brought_back": done.summary.created,
    })
}

pub fn deleted(config: &AreaName, id: SnapshotId) -> String {
    format!("deleted #{id} from {config}\n")
}

pub fn deleted_json(config: &AreaName, id: SnapshotId) -> Value {
    json!({"config": config.as_str(), "deleted": id.get()})
}
