// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Listing snapshots and the history of a file.

use serde_json::{json, Value};
use snapall_core::{AreaName, FileHistory, SnapshotInfo, SnapshotKind};

pub fn history(history: &FileHistory) -> String {
    let mut out = format!("{}  (config {})\n", history.path, history.area);
    if history.versions.is_empty() {
        out.push_str("no snapshots of this area yet\n");
        return out;
    }
    let width = history
        .versions
        .iter()
        .map(|v| v.snapshot.id.to_string().len())
        .max()
        .unwrap_or(1)
        .max(1);
    out.push_str(&format!(
        "{:>width$}  {:<19}  {:<16}  description\n",
        "#", "date", "state"
    ));
    for version in &history.versions {
        out.push_str(&format!(
            "{:>width$}  {:<19}  {:<16}  {}\n",
            version.snapshot.id,
            version.snapshot.date.as_deref().unwrap_or("-"),
            version.state.label(),
            version.snapshot.description
        ));
    }
    out
}

pub fn history_json(history: &FileHistory) -> Value {
    let versions: Vec<Value> = history
        .versions
        .iter()
        .map(|v| {
            json!({
                "snapshot": v.snapshot.id.get(),
                "date": v.snapshot.date,
                "description": v.snapshot.description,
                "state": v.state.token(),
                "restorable": v.state.restorable(),
            })
        })
        .collect();
    json!({"path": history.path, "config": history.area.as_str(), "versions": versions})
}

pub fn protected_areas(configs: &[AreaName]) -> String {
    let mut sorted: Vec<&AreaName> = configs.iter().collect();
    sorted.sort();
    sorted.iter().map(|c| format!("{c}\n")).collect()
}

pub fn areas_json(configs: &[AreaName]) -> Value {
    let mut sorted: Vec<&str> = configs.iter().map(AreaName::as_str).collect();
    sorted.sort_unstable();
    json!({ "configs": sorted })
}

pub(super) fn kind_label(info: &SnapshotInfo) -> String {
    match info.kind {
        SnapshotKind::Single => "single".to_owned(),
        SnapshotKind::Pre => "pre".to_owned(),
        SnapshotKind::Post { pre } => format!("post<-{pre}"),
    }
}

/// One block per area: a header, then an aligned table, newest snapshot last.
pub fn snapshots(all: &[(AreaName, Vec<SnapshotInfo>)]) -> String {
    let mut blocks = Vec::new();
    for (config, list) in all {
        let mut block = format!("{config}\n");
        if list.is_empty() {
            block.push_str("  (no snapshots)\n");
        } else {
            let rows: Vec<[String; 5]> = list
                .iter()
                .map(|s| {
                    [
                        s.id.to_string(),
                        kind_label(s),
                        s.date.clone().unwrap_or_default(),
                        s.cleanup.clone().unwrap_or_default(),
                        s.description.clone(),
                    ]
                })
                .collect();
            let header = ["#", "type", "date", "cleanup", "description"].map(str::to_owned);
            let mut widths = [0usize; 4];
            for row in std::iter::once(&header).chain(rows.iter()) {
                for (width, cell) in widths.iter_mut().zip(row.iter()) {
                    *width = (*width).max(cell.chars().count());
                }
            }
            for row in std::iter::once(&header).chain(rows.iter()) {
                block.push_str(&format!(
                    "  {:>w0$}  {:<w1$}  {:<w2$}  {:<w3$}  {}\n",
                    row[0],
                    row[1],
                    row[2],
                    row[3],
                    row[4],
                    w0 = widths[0],
                    w1 = widths[1],
                    w2 = widths[2],
                    w3 = widths[3],
                ));
            }
        }
        blocks.push(block);
    }
    blocks.join("\n")
}

pub fn snapshots_json(all: &[(AreaName, Vec<SnapshotInfo>)]) -> Value {
    let configs: serde_json::Map<String, Value> = all
        .iter()
        .map(|(config, list)| {
            let list: Vec<Value> = list
                .iter()
                .map(|s| {
                    let mut value = json!({
                        "id": s.id.get(),
                        "type": match s.kind {
                            SnapshotKind::Single => "single",
                            SnapshotKind::Pre => "pre",
                            SnapshotKind::Post { .. } => "post",
                        },
                        "date": s.date,
                        "user": s.user,
                        "cleanup": s.cleanup,
                        "description": s.description,
                    });
                    if let SnapshotKind::Post { pre } = s.kind {
                        value["pre"] = json!(pre.get());
                    }
                    value
                })
                .collect();
            (config.to_string(), Value::Array(list))
        })
        .collect();
    json!({ "configs": configs })
}
