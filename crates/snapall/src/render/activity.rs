// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall has done, as the journal remembers it.

use serde_json::{json, Value};

pub(super) fn took_words(took_ms: Option<u64>) -> String {
    match took_ms {
        None => String::new(),
        Some(ms) if ms < 1000 => format!(" ({ms} ms)"),
        Some(ms) => format!(" ({:.1} s)", ms as f64 / 1000.0),
    }
}

/// What snapall did, one line each, newest first; a change that failed or was cut short says why beneath.
pub fn activity(entries: &[snapall_core::JournalEntry]) -> String {
    use snapall_core::{JournalStatus, Source};
    if entries.is_empty() {
        return "Nothing has been recorded yet: taking, deleting or restoring a snapshot is.\n"
            .to_owned();
    }
    let mut out = String::new();
    for entry in entries {
        let when = entry
            .finished_at
            .as_deref()
            .or(entry.started_at.as_deref())
            .unwrap_or("-");
        let origin = match entry.source {
            Some(Source::Window) => "window",
            Some(Source::Command) => "command",
            None => "-",
        };
        let step = match (&entry.status, &entry.progress) {
            (JournalStatus::Running, Some(p)) => {
                format!(" (step {} of {}: {})", p.done + 1, p.of, p.area)
            }
            _ => String::new(),
        };
        out.push_str(&format!(
            "{when:<19}  {:<9} {origin:<8} {}{step}{}\n",
            entry.status.word(),
            entry.what,
            took_words(entry.took_ms)
        ));
        if matches!(
            entry.status,
            JournalStatus::Failed | JournalStatus::Interrupted | JournalStatus::Cancelled
        ) {
            out.push_str(&format!("                     {}\n", entry.detail));
        }
        for line in entry
            .rollback
            .iter()
            .flat_map(|way_back| way_back.words(&entry.id))
        {
            out.push_str(&format!("                     {line}\n"));
        }
    }
    out
}

pub fn activity_json(entries: &[snapall_core::JournalEntry]) -> Value {
    json!(entries
        .iter()
        .map(|entry| json!({
            "id": entry.id,
            "what": entry.what,
            "status": entry.status.word().to_lowercase(),
            "source": entry.source.map(snapall_core::Source::word),
            "started_at": entry.started_at,
            "finished_at": entry.finished_at,
            "took_ms": entry.took_ms,
            "detail": entry.detail,
            "pid": entry.pid,
            "progress": entry.progress.as_ref().map(|p| json!({"done": p.done, "of": p.of, "area": p.area})),
            "rollback": entry.rollback.as_ref().map(|way_back| match way_back {
                snapall_core::Rollback::Undo(undo) => json!({
                    "kind": "undo", "what": undo.what, "weight": undo.weight.word(), "commands": undo.commands,
                }),
                snapall_core::Rollback::Advice(text) => json!({"kind": "advice", "text": text}),
            }),
        }))
        .collect::<Vec<_>>())
}
