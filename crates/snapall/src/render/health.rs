// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How protection stands, and the checks that find out.

use super::*;
use serde_json::{json, Value};
use snapall_core::protecting::doctor::{Check, OsRelease, Status};

pub(super) fn level_mark(level: snapall_core::Level) -> &'static str {
    use snapall_core::Level;
    match level {
        Level::Problem => "!!",
        Level::Attention => "! ",
        Level::Note => "- ",
        Level::Good => "ok",
    }
}

pub(super) fn level_word(level: snapall_core::Level) -> &'static str {
    use snapall_core::Level;
    match level {
        Level::Problem => "problem",
        Level::Attention => "attention",
        Level::Note => "note",
        Level::Good => "good",
    }
}

pub(super) fn topic_word(topic: snapall_core::Topic) -> &'static str {
    use snapall_core::Topic;
    match topic {
        Topic::Space => "space",
        Topic::Cleanup => "cleanup",
        Topic::BootMenu => "boot-menu",
        Topic::Snapshots => "snapshots",
        Topic::Jobs => "jobs",
    }
}

/// Words broken into lines of at most `width`, each after the first indented.
pub(super) fn wrapped(text: &str, width: usize, indent: &str) -> String {
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
            area.location,
            area.snapshots,
            if area.snapshots == 1 { "" } else { "s" },
            area.newest.as_deref().unwrap_or("none yet"),
            area.lives_in()
        ));
    }
    out.push_str("\nBackground jobs:\n");
    for job in &facts.jobs {
        let plugin =
            job.job.unit == snapall_core::keeping::jobs::BOOT_MENU.unit && facts.boot_menu_plugin;
        if !job.state.is_installed()
            && !plugin
            && job.job.unit != snapall_core::keeping::jobs::CLEANUP.unit
        {
            continue; // not on this computer: nothing to say
        }
        let word = if plugin {
            "kept up to date by a plugin"
        } else {
            job.state.words()
        };
        let when = job
            .details
            .as_ref()
            .map(|d| d.schedule().join(", "))
            .filter(|w| !w.is_empty());
        out.push_str(&format!(
            "  {}: {word}\n      {}",
            job.job.title, job.job.unit
        ));
        if let Some(description) = job.details.as_ref().and_then(|d| d.description.as_deref()) {
            out.push_str(&format!(" (\"{description}\")"));
        }
        if let Some(when) = when {
            out.push_str(&format!(", runs {when}"));
        }
        out.push('\n');
    }
    if let Some(area) = facts.areas.first() {
        let word = match area.quotas {
            snapall_core::QuotaMode::Off => "off",
            snapall_core::QuotaMode::Full => "on",
            snapall_core::QuotaMode::Simple => "on, the simple kind",
            snapall_core::QuotaMode::Unknown => "could not be found out",
        };
        out.push_str(&format!("\nCounting the room snapshots take: {word}\n"));
    }
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
            "path": a.location,
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

pub fn setup_json(report: &crate::setup::Report) -> Value {
    let worth = |worth: snapall_core::protecting::coverage::Worth| match worth {
        snapall_core::protecting::coverage::Worth::Recommended => "recommended",
        snapall_core::protecting::coverage::Worth::Optional => "optional",
        snapall_core::protecting::coverage::Worth::NotUseful => "not_useful",
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
            snapall_core::vocabulary::explain::Cleanup::On => "on",
            snapall_core::vocabulary::explain::Cleanup::Off => "off",
            snapall_core::vocabulary::explain::Cleanup::Unavailable => "unavailable",
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
    let worst = snapall_core::protecting::doctor::worst(checks);
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
