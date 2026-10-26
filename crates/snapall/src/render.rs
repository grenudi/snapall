// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation: pure functions from domain values to text or JSON.

use serde_json::{json, Value};
use snapall_core::doctor::{Check, OsRelease, Status};
use snapall_core::{
    ChangeKind, ConfigName, Description, FileHistory, Plan, Preview, Report, Restored, SnapshotId,
    SnapshotInfo, SnapshotKind,
};

pub fn report(report: &Report) -> String {
    let width = report
        .outcomes
        .iter()
        .map(|o| o.config.as_str().len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for outcome in &report.outcomes {
        match &outcome.result {
            Ok(id) => out.push_str(&format!("ok    {:<width$}  #{id}\n", outcome.config)),
            Err(err) => out.push_str(&format!("FAIL  {:<width$}  {err}\n", outcome.config)),
        }
    }
    out
}

pub fn report_json(report: &Report) -> Value {
    let outcomes: Vec<Value> = report
        .outcomes
        .iter()
        .map(|o| match &o.result {
            Ok(id) => json!({"config": o.config.as_str(), "snapshot": id.get()}),
            Err(err) => json!({"config": o.config.as_str(), "error": err.to_string()}),
        })
        .collect();
    json!({"ok": report.is_success(), "outcomes": outcomes})
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Warn => "warn",
        Status::Fail => "FAIL",
    }
}

/// What a restore would do to one path, in words that say what *happens*, not how it differs.
fn verb(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Changed => "revert",
        ChangeKind::Added => "remove",
        ChangeKind::Removed => "bring back",
    }
}

fn kind_word(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Changed => "changed",
        ChangeKind::Added => "added",
        ChangeKind::Removed => "removed",
    }
}

/// How many changed paths a human sees before the rest is summarised. `--json` is never cut.
const SHOWN: usize = 200;

pub fn changes(config: &ConfigName, id: SnapshotId, preview: &Preview) -> String {
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

pub fn changes_json(config: &ConfigName, id: SnapshotId, preview: &Preview) -> Value {
    let changes: Vec<Value> = preview
        .changes
        .iter()
        .map(|c| json!({"kind": kind_word(c.kind), "path": c.path}))
        .collect();
    json!({
        "config": config.as_str(),
        "snapshot": id.get(),
        "subvolume": preview.subvolume,
        "system_root": preview.is_system_root,
        "changes": changes,
    })
}

pub fn restored(config: &ConfigName, id: SnapshotId, done: &Restored) -> String {
    let s = done.summary;
    format!(
        "restored {config} to #{id}: {} reverted, {} removed, {} brought back\n\
         undo this with: snapall restore {config} {}  (the state before is snapshot #{})\n",
        s.modified, s.deleted, s.created, done.safety_snapshot, done.safety_snapshot
    )
}

pub fn restored_json(config: &ConfigName, id: SnapshotId, done: &Restored) -> Value {
    json!({
        "config": config.as_str(),
        "snapshot": id.get(),
        "safety_snapshot": done.safety_snapshot.get(),
        "reverted": done.summary.modified,
        "removed": done.summary.deleted,
        "brought_back": done.summary.created,
    })
}

pub fn deleted(config: &ConfigName, id: SnapshotId) -> String {
    format!("deleted #{id} from {config}\n")
}

pub fn deleted_json(config: &ConfigName, id: SnapshotId) -> Value {
    json!({"config": config.as_str(), "deleted": id.get()})
}

pub fn history(history: &FileHistory) -> String {
    let mut out = format!("{}  (config {})\n", history.path, history.config);
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
    json!({"path": history.path, "config": history.config.as_str(), "versions": versions})
}

// ---- health -------------------------------------------------------------------------------------

fn level_mark(level: snapall_core::Level) -> &'static str {
    use snapall_core::Level;
    match level {
        Level::Problem => "!!",
        Level::Attention => "! ",
        Level::Note => "- ",
        Level::Good => "ok",
    }
}

fn level_word(level: snapall_core::Level) -> &'static str {
    use snapall_core::Level;
    match level {
        Level::Problem => "problem",
        Level::Attention => "attention",
        Level::Note => "note",
        Level::Good => "good",
    }
}

fn topic_word(topic: snapall_core::Topic) -> &'static str {
    use snapall_core::Topic;
    match topic {
        Topic::Space => "space",
        Topic::Cleanup => "cleanup",
        Topic::BootMenu => "boot-menu",
        Topic::Snapshots => "snapshots",
    }
}

/// Words broken into lines of at most `width`, each after the first indented.
fn wrapped(text: &str, width: usize, indent: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    lines.join(&format!("\n{indent}"))
}

/// What is wrong, if anything, and what to do; then the numbers behind it.
pub fn health(facts: &snapall_core::Facts, report: &snapall_core::Health) -> String {
    use snapall_core::Remedy;
    let mut out = format!("Health: {}\n", report.verdict.text);
    for finding in &report.findings {
        out.push_str(&format!(
            "\n  {}  {}\n      {}\n",
            level_mark(finding.level),
            finding.headline,
            wrapped(&finding.why, 70, "      ")
        ));
        if let Some(Remedy::SwitchOnCleanup) = finding.remedy {
            out.push_str("      To fix it: sudo snapall setup (it offers to switch cleanup on)\n");
        }
    }
    out.push_str("\nThe areas:\n");
    for area in &facts.areas {
        let room = area
            .measured_bytes
            .map_or("room not measured".to_owned(), |b| {
                format!("{} held", snapall_core::health::format_bytes(b))
            });
        let disk = area.disk.map_or(String::new(), |d| {
            format!("   disk {}% full", d.percent_used())
        });
        let keeps = area
            .retention
            .as_ref()
            .and_then(|r| r.keep)
            .map_or(String::new(), |k| format!("   keeps the newest {k}"));
        out.push_str(&format!(
            "  {} ({})\n      {} snapshot{}, {room}{disk}{keeps}\n      newest: {}   kept in {}\n",
            area.title(),
            area.subvolume,
            area.snapshots,
            if area.snapshots == 1 { "" } else { "s" },
            area.newest.as_deref().unwrap_or("none yet"),
            area.lives_in()
        ));
    }
    out.push_str("\nBackground jobs:\n");
    let cleanup = match snapall_core::explain::cleanup_from(
        facts.cleanup_timer.enabled.as_deref(),
        facts.cleanup_timer.active.as_deref(),
    ) {
        snapall_core::explain::Cleanup::On => "on",
        snapall_core::explain::Cleanup::Off => "off",
        snapall_core::explain::Cleanup::Unavailable => "could not be found out",
    };
    let boot = match (&facts.boot_menu_sync, facts.boot_menu_plugin) {
        (_, true) => "kept up to date by a plugin",
        (sync, false) if sync.is_running() => "running",
        (sync, false) if sync.is_installed() => "installed but not running",
        _ => "not used on this computer",
    };
    out.push_str(&format!(
        "  cleaning up old snapshots   {cleanup}\n  the boot menu following snapshots   {boot}\n"
    ));
    out
}

pub fn health_json(facts: &snapall_core::Facts, report: &snapall_core::Health) -> Value {
    json!({
        "verdict": {"level": level_word(report.verdict.level), "text": report.verdict.text},
        "findings": report.findings.iter().map(|f| json!({
            "level": level_word(f.level),
            "topic": topic_word(f.topic),
            "headline": f.headline,
            "why": f.why,
            "can_fix": f.remedy.is_some(),
        })).collect::<Vec<_>>(),
        "status": report.chips.iter().map(|c| json!({"label": c.label, "level": level_word(c.level), "topic": topic_word(c.topic)})).collect::<Vec<_>>(),
        "areas": facts.areas.iter().map(|a| json!({
            "area": a.config.as_str(),
            "path": a.subvolume,
            "snapshots": a.snapshots,
            "oldest": a.oldest,
            "newest": a.newest,
            "bytes_held": a.measured_bytes,
            "kept_in": a.lives_in(),
            "disk_percent_used": a.disk.map(|d| d.percent_used()),
            "keeps": a.retention.as_ref().and_then(|r| r.keep),
        })).collect::<Vec<_>>(),
    })
}

// ---- restore points -----------------------------------------------------------------------------

fn areas_of(point: &snapall_core::RestorePoint) -> String {
    point
        .members
        .iter()
        .map(|m| m.config.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The restore points, newest first, each with what to type to go back to it.
pub fn points(points: &[snapall_core::RestorePoint]) -> String {
    if points.is_empty() {
        return "No restore points yet. `sudo snapall` takes a snapshot of every area at once, and that is a restore point.\n".to_owned();
    }
    let mut out = String::from(
        "Restore points: the snapshots that one action took together, one in every area.\nPut every area back to one with `sudo snapall restore-all POINT`.\n\n",
    );
    let width =
        |f: &dyn Fn(&snapall_core::RestorePoint) -> usize| points.iter().map(f).max().unwrap_or(0);
    let w_point = width(&|p| p.point.as_str().len()).max("POINT".len());
    let w_date = width(&|p| p.date.as_deref().unwrap_or("").len()).max("WHEN".len());
    let w_areas = width(&|p| areas_of(p).len()).max("AREAS".len());
    out.push_str(&format!(
        "{:<w_point$}  {:<w_date$}  {:<w_areas$}  NAME\n",
        "POINT", "WHEN", "AREAS"
    ));
    for p in points {
        let note = if p.tagged {
            ""
        } else {
            "   (older: matched by name and time)"
        };
        out.push_str(&format!(
            "{:<w_point$}  {:<w_date$}  {:<w_areas$}  {}{note}\n",
            p.point.as_str(),
            p.date.as_deref().unwrap_or(""),
            areas_of(p),
            p.description
        ));
    }
    out
}

pub fn points_json(points: &[snapall_core::RestorePoint]) -> Value {
    json!({ "points": points.iter().map(|p| json!({
        "point": p.point.as_str(),
        "description": p.description,
        "date": p.date,
        "tagged": p.tagged,
        "areas": p.members.iter().map(|m| json!({"area": m.config.as_str(), "snapshot": m.id.get()})).collect::<Vec<_>>(),
    })).collect::<Vec<_>>() })
}

fn path_counts(changes: &[snapall_core::Change]) -> (usize, usize, usize) {
    use snapall_core::ChangeKind;
    let count = |kind: ChangeKind| changes.iter().filter(|c| c.kind == kind).count();
    (
        count(ChangeKind::Changed),
        count(ChangeKind::Added),
        count(ChangeKind::Removed),
    )
}

/// What putting every area of a point back would do, area by area, and why the system is left out.
pub fn point_preview(
    point: &snapall_core::RestorePoint,
    previews: &[snapall_core::MemberPreview],
    system_route: &str,
) -> String {
    use snapall_core::MemberView;
    let mut out = format!(
        "Restore point {} ({}), taken {}. Putting every area back would:\n\n",
        point.point,
        point.description,
        point.date.as_deref().unwrap_or("at an unknown time")
    );
    let width = previews
        .iter()
        .map(|p| p.config.as_str().len())
        .max()
        .unwrap_or(0);
    let mut system = false;
    for p in previews {
        let what = match &p.view {
            MemberView::Changes(c) if c.changes.is_empty() => {
                "nothing to change: it already matches".to_owned()
            }
            MemberView::Changes(c) => {
                let (changed, added, removed) = path_counts(&c.changes);
                format!(
                    "change {} path{}: {changed} reverted, {added} removed, {removed} brought back",
                    c.changes.len(),
                    if c.changes.len() == 1 { "" } else { "s" }
                )
            }
            MemberView::SystemRoot => {
                system = true;
                format!(
                    "not touched: the running system is not put back while it runs (its snapshot in this point is #{})",
                    p.id
                )
            }
            MemberView::Failed(why) => format!("could not be looked at: {why}"),
        };
        out.push_str(&format!("  {:<width$}  {what}\n", p.config.as_str()));
    }
    if system {
        out.push_str(&format!(
            "\nThe system itself: {system_route}\n\nTo bring the system back as well, do that first, then run restore-all with this point again: the system will already match, and the other areas follow.\n"
        ));
    }
    out
}

pub fn point_preview_json(
    point: &snapall_core::RestorePoint,
    previews: &[snapall_core::MemberPreview],
) -> Value {
    use snapall_core::MemberView;
    json!({
        "point": point.point.as_str(),
        "areas": previews.iter().map(|p| match &p.view {
            MemberView::Changes(c) => json!({"area": p.config.as_str(), "snapshot": p.id.get(), "action": if c.changes.is_empty() { "unchanged" } else { "change" }, "paths": c.changes.len()}),
            MemberView::SystemRoot => json!({"area": p.config.as_str(), "snapshot": p.id.get(), "action": "system"}),
            MemberView::Failed(why) => json!({"area": p.config.as_str(), "snapshot": p.id.get(), "action": "error", "error": why}),
        }).collect::<Vec<_>>(),
    })
}

/// What putting a point back did, and the one command that undoes all of it.
pub fn point_restored(
    point: &snapall_core::RestorePoint,
    done: &snapall_core::PointRestore,
) -> String {
    use snapall_core::MemberResult;
    let mut out = format!("Restore point {} ({}):\n\n", point.point, point.description);
    let width = done
        .members
        .iter()
        .map(|m| m.config.as_str().len())
        .max()
        .unwrap_or(0);
    for m in &done.members {
        let what = match &m.result {
            MemberResult::Restored(r) => format!(
                "put back ({} path{}); the state before is snapshot #{}",
                r.summary.created + r.summary.modified + r.summary.deleted,
                if r.summary.created + r.summary.modified + r.summary.deleted == 1 {
                    ""
                } else {
                    "s"
                },
                r.safety_snapshot
            ),
            MemberResult::AlreadyThere => "already matched: left alone".to_owned(),
            MemberResult::SystemRoot => {
                format!(
                "left alone: the running system is not put back while it runs (its snapshot in this point is #{})",
                m.id
            )
            }
            MemberResult::Failed(why) => format!("FAILED: {why}"),
        };
        out.push_str(&format!("  {:<width$}  {what}\n", m.config.as_str()));
    }
    if done.changed_anything() {
        out.push_str(&format!(
            "\nTo undo all of this at once: sudo snapall restore-all {}\n",
            done.safety
        ));
    } else {
        out.push_str("\nNothing was changed.\n");
    }
    out
}

pub fn point_restored_json(
    point: &snapall_core::RestorePoint,
    done: &snapall_core::PointRestore,
) -> Value {
    use snapall_core::MemberResult;
    json!({
        "point": point.point.as_str(),
        "undo_point": done.changed_anything().then(|| done.safety.as_str().to_owned()),
        "areas": done.members.iter().map(|m| match &m.result {
            MemberResult::Restored(r) => json!({"area": m.config.as_str(), "result": "restored", "snapshot": m.id.get(), "safety_snapshot": r.safety_snapshot.get(), "paths": r.summary.created + r.summary.modified + r.summary.deleted}),
            MemberResult::AlreadyThere => json!({"area": m.config.as_str(), "result": "unchanged", "snapshot": m.id.get()}),
            MemberResult::SystemRoot => json!({"area": m.config.as_str(), "result": "system", "snapshot": m.id.get()}),
            MemberResult::Failed(why) => json!({"area": m.config.as_str(), "result": "failed", "snapshot": m.id.get(), "error": why}),
        }).collect::<Vec<_>>(),
    })
}

pub fn setup_json(report: &crate::setup::Report) -> Value {
    let worth = |worth: snapall_core::coverage::Worth| match worth {
        snapall_core::coverage::Worth::Recommended => "recommended",
        snapall_core::coverage::Worth::Optional => "optional",
        snapall_core::coverage::Worth::NotUseful => "not_useful",
    };
    json!({
        "dry_run": report.dry_run,
        "areas": report.areas.iter().map(|a| json!({
            "mount_point": a.mount_point,
            "title": a.title,
            "protected": a.protected_by.is_some(),
            "config": a.protected_by.as_ref().map(|c| c.as_str()),
            "worth": worth(a.worth),
        })).collect::<Vec<_>>(),
        "protected_now": report.protected_now.iter().map(|(point, name)| json!({"mount_point": point, "config": name.as_str()})).collect::<Vec<_>>(),
        "declined": report.declined,
        "failed": report.failed.iter().map(|(what, why)| json!({"what": what, "why": why})).collect::<Vec<_>>(),
        "cleanup": match report.cleanup {
            snapall_core::explain::Cleanup::On => "on",
            snapall_core::explain::Cleanup::Off => "off",
            snapall_core::explain::Cleanup::Unavailable => "unavailable",
        },
        "cleanup_switched_on": report.cleanup_switched_on,
    })
}

pub fn doctor(os: &OsRelease, checks: &[Check]) -> String {
    let mut out = format!("system: {}\n", os.display_name());
    for check in checks {
        out.push_str(&format!(
            "{:<5} {:<11} {}\n",
            status_label(check.status),
            check.name,
            check.detail
        ));
        if let Some(fix) = &check.fix {
            out.push_str(&format!("      {:<11} fix: {fix}\n", ""));
        }
    }
    out
}

pub fn doctor_json(os: &OsRelease, checks: &[Check]) -> Value {
    let worst = snapall_core::doctor::worst(checks);
    json!({
        "system": os.display_name(),
        "ok": worst != Status::Fail,
        "checks": checks.iter().map(|c| json!({
            "name": c.name,
            "status": status_label(c.status).to_lowercase(),
            "detail": c.detail,
            "fix": c.fix,
        })).collect::<Vec<_>>(),
    })
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
        "would_snapshot": plan.targets().iter().map(ConfigName::as_str).collect::<Vec<_>>(),
    })
}

pub fn configs(configs: &[ConfigName]) -> String {
    let mut sorted: Vec<&ConfigName> = configs.iter().collect();
    sorted.sort();
    sorted.iter().map(|c| format!("{c}\n")).collect()
}

pub fn configs_json(configs: &[ConfigName]) -> Value {
    let mut sorted: Vec<&str> = configs.iter().map(ConfigName::as_str).collect();
    sorted.sort_unstable();
    json!({ "configs": sorted })
}

fn kind_label(info: &SnapshotInfo) -> String {
    match info.kind {
        SnapshotKind::Single => "single".to_owned(),
        SnapshotKind::Pre => "pre".to_owned(),
        SnapshotKind::Post { pre } => format!("post<-{pre}"),
    }
}

/// One block per config: a header, then an aligned table, newest snapshot last.
pub fn snapshots(all: &[(ConfigName, Vec<SnapshotInfo>)]) -> String {
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

pub fn snapshots_json(all: &[(ConfigName, Vec<SnapshotInfo>)]) -> Value {
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

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::testing::InMemoryBackend;
    use snapall_core::{all_snapshots, plan_snapshots, snapshot_all, SnapshotBackend};

    #[test]
    fn report_lists_successes_and_failures_aligned() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]).failing_on("home");
        let r = snapshot_all(&backend, None, &Description::default(), None).unwrap();
        assert_eq!(
            report(&r),
            "FAIL  home  `in-memory` failed (injected failure): snapshot of home refused\nok    root  #1\n"
        );
        let value = report_json(&r);
        assert_eq!(value["ok"], false);
        assert_eq!(value["outcomes"][1]["snapshot"], 1);
        assert!(value["outcomes"][0]["error"]
            .as_str()
            .unwrap()
            .contains("refused"));
    }

    #[test]
    fn dry_run_names_every_target_in_both_formats() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        let description = Description::parse("x").unwrap();
        let text = dry_run(&plan, &description);
        assert!(text.contains("would snapshot 2 area(s)"));
        assert!(text.contains("  home\n") && text.contains("  root\n"));
        let value = dry_run_json(&plan, &description);
        assert_eq!(value["would_snapshot"], json!(["home", "root"]));
        assert_eq!(value["dry_run"], true);
    }

    #[test]
    fn configs_are_sorted_in_both_formats() {
        let names = [
            ConfigName::parse("root").unwrap(),
            ConfigName::parse("home").unwrap(),
        ];
        assert_eq!(configs(&names), "home\nroot\n");
        assert_eq!(configs_json(&names), json!({"configs": ["home", "root"]}));
    }

    #[test]
    fn snapshot_tables_align_and_empty_configs_say_so() {
        let backend = InMemoryBackend::with_configs(&["home", "root"]);
        let home = ConfigName::parse("home").unwrap();
        for text in ["before theming", "after theming"] {
            backend
                .create_snapshot(&home, &Description::parse(text).unwrap())
                .unwrap();
        }
        let all = all_snapshots(&backend).unwrap();
        let text = snapshots(&all);
        assert_eq!(
            text,
            "home\n  \
             #  type    date                 cleanup  description\n  \
             1  single  2026-01-01 00:00:01  number   before theming\n  \
             2  single  2026-01-01 00:00:02  number   after theming\n\
             \nroot\n  (no snapshots)\n"
        );
        let value = snapshots_json(&all);
        assert_eq!(value["configs"]["home"][1]["description"], "after theming");
        assert_eq!(value["configs"]["home"][0]["type"], "single");
        assert_eq!(value["configs"]["root"], json!([]));
    }

    #[test]
    fn doctor_output_shows_status_detail_and_fix() {
        use snapall_core::doctor::{diagnose, Probe};
        let probe = Probe {
            os: OsRelease::parse("ID=arch\nPRETTY_NAME=\"Arch Linux\"\n"),
            snapper: Err("No such file or directory".into()),
            is_root: Some(false),
            configs: None,
            subvolumes: Vec::new(),
        };
        let checks = diagnose(&probe);
        let text = doctor(&probe.os, &checks);
        assert!(text.starts_with("system: Arch Linux\n"), "{text}");
        assert!(text.contains("FAIL  snapper"), "{text}");
        assert!(
            text.contains("fix: pacman -S --needed --noconfirm snapper"),
            "{text}"
        );
        assert!(text.contains("warn  privileges"), "{text}");
        let value = doctor_json(&probe.os, &checks);
        assert_eq!(value["ok"], false);
        assert_eq!(value["checks"][0]["status"], "fail");
        assert_eq!(value["checks"][1]["fix"], "run with sudo");
    }

    // ---- restore points ------------------------------------------------------------------------

    fn points_fixture() -> (
        snapall_core::testing::InMemoryBackend,
        snapall_core::RestorePoint,
    ) {
        use snapall_core::testing::InMemoryBackend;
        let backend =
            InMemoryBackend::with_configs(&["docs", "home", "root"]).with_root_config("root");
        backend.write_file("docs", "/mnt/docs/a.txt", "then");
        backend.write_file("home", "/mnt/home/b.txt", "then");
        let plan = snapall_core::plan_snapshots(&backend, None).unwrap();
        let point = snapall_core::Point::parse("20261025T100000Z").unwrap();
        snapall_core::run_plan(
            &backend,
            &plan,
            &Description::parse("before theming").unwrap(),
            Some(&point),
        );
        let found = snapall_core::restore_points(&snapall_core::all_snapshots(&backend).unwrap());
        (backend, found.into_iter().next().unwrap())
    }

    #[test]
    fn no_restore_points_says_what_makes_one() {
        let text = points(&[]);
        assert!(
            text.contains("No restore points yet") && text.contains("sudo snapall"),
            "{text}"
        );
        assert_eq!(points_json(&[]), json!({"points": []}));
    }

    #[test]
    fn restore_points_are_a_table_with_the_command_that_uses_them() {
        let (_, point) = points_fixture();
        let text = points(std::slice::from_ref(&point));
        assert!(text.contains("sudo snapall restore-all POINT"), "{text}");
        assert!(
            text.contains("POINT")
                && text.contains("WHEN")
                && text.contains("AREAS")
                && text.contains("NAME"),
            "{text}"
        );
        assert!(
            text.contains("20261025T100000Z")
                && text.contains("docs, home, root")
                && text.contains("before theming"),
            "{text}"
        );
        assert!(
            !text.contains("matched"),
            "a tagged point is not marked as a guess: {text}"
        );
        let value = points_json(std::slice::from_ref(&point));
        assert_eq!(value["points"][0]["point"], "20261025T100000Z");
        assert_eq!(value["points"][0]["tagged"], true);
        assert_eq!(
            value["points"][0]["areas"][1],
            json!({"area": "home", "snapshot": 1})
        );
    }

    #[test]
    fn a_matched_point_is_marked_as_older_and_a_guess() {
        let (_, mut point) = points_fixture();
        point.tagged = false;
        assert!(points(&[point]).contains("older: matched by name and time"));
    }

    #[test]
    fn the_preview_says_per_area_what_changes_and_why_the_system_is_left_out() {
        let (backend, point) = points_fixture();
        backend.write_file("home", "/mnt/home/b.txt", "ruined");
        let text = point_preview(
            &point,
            &snapall_core::preview_point(&backend, &point),
            "boot from a live USB",
        );
        assert!(
            text.contains("docs") && text.contains("nothing to change: it already matches"),
            "{text}"
        );
        assert!(
            text.contains("change 1 path: 1 reverted, 0 removed, 0 brought back"),
            "{text}"
        );
        assert!(
            text.contains("not touched: the running system is not put back while it runs"),
            "{text}"
        );
        assert!(
            text.contains("The system itself: boot from a live USB"),
            "{text}"
        );
        let value = point_preview_json(&point, &snapall_core::preview_point(&backend, &point));
        let actions: Vec<&str> = value["areas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["action"].as_str().unwrap())
            .collect();
        assert_eq!(actions, ["unchanged", "change", "system"]);
    }

    #[test]
    fn what_was_done_says_each_area_and_the_one_command_that_undoes_all_of_it_only_if_something_changed(
    ) {
        let (backend, point) = points_fixture();
        backend.write_file("home", "/mnt/home/b.txt", "ruined");
        let done = snapall_core::restore_point(
            &backend,
            &point,
            snapall_core::Point::parse("20261025T110000Z").unwrap(),
        );
        let text = point_restored(&point, &done);
        assert!(
            text.contains("put back (1 path)")
                && text.contains("already matched: left alone")
                && text.contains("left alone: the running system"),
            "{text}"
        );
        assert!(
            text.contains("To undo all of this at once: sudo snapall restore-all 20261025T110000Z"),
            "{text}"
        );
        assert_eq!(
            point_restored_json(&point, &done)["undo_point"],
            "20261025T110000Z"
        );
        // nothing differs now, so nothing is done and there is nothing to undo
        let nothing = snapall_core::restore_point(
            &backend,
            &point,
            snapall_core::Point::parse("20261025T120000Z").unwrap(),
        );
        let text = point_restored(&point, &nothing);
        assert!(
            text.contains("Nothing was changed.") && !text.contains("To undo"),
            "{text}"
        );
        assert_eq!(
            point_restored_json(&point, &nothing)["undo_point"],
            Value::Null
        );
    }
}
