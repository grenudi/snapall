// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What taking a snapshot reports: what was taken, or what would be.

use serde_json::{json, Value};
use snapall_core::doctor::Status;
use snapall_core::{AreaName, ChangeKind, Description, Plan, Report};

pub fn report(report: &Report) -> String {
    let width = report
        .outcomes
        .iter()
        .map(|o| o.area.as_str().len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for outcome in &report.outcomes {
        match &outcome.result {
            Ok(id) => out.push_str(&format!("ok    {:<width$}  #{id}\n", outcome.area)),
            Err(err) => out.push_str(&format!("FAIL  {:<width$}  {err}\n", outcome.area)),
        }
    }
    out
}

pub fn report_json(report: &Report) -> Value {
    let outcomes: Vec<Value> = report
        .outcomes
        .iter()
        .map(|o| match &o.result {
            Ok(id) => json!({"config": o.area.as_str(), "snapshot": id.get()}),
            Err(err) => json!({"config": o.area.as_str(), "error": err.to_string()}),
        })
        .collect();
    json!({"ok": report.is_success(), "outcomes": outcomes})
}

pub(super) fn status_label(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Warn => "warn",
        Status::Fail => "FAIL",
    }
}

/// What a restore would do to one path, in words that say what *happens*, not how it differs.
pub(super) fn verb(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Changed => "revert",
        ChangeKind::Added => "remove",
        ChangeKind::Removed => "bring back",
    }
}

pub(super) fn kind_word(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Changed => "changed",
        ChangeKind::Added => "added",
        ChangeKind::Removed => "removed",
    }
}

pub fn dry_run(plan: &Plan, description: &Description) -> String {
    let mut out = format!(
        "dry run: would snapshot {} area(s) with description \"{description}\"\n",
        plan.targets().len()
    );
    for config in plan.targets() {
        out.push_str(&format!("  {config}\n"));
    }
    out
}

pub fn dry_run_json(plan: &Plan, description: &Description) -> Value {
    json!({
        "dry_run": true,
        "description": description.as_str(),
        "would_snapshot": plan.targets().iter().map(AreaName::as_str).collect::<Vec<_>>(),
    })
}
