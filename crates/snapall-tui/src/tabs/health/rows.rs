// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the Health tab lists: each row's name, how it stands, what it means and what Enter does.
//! A pure function of the report, so it is tested without a terminal.

use super::*;
use snapall_core::explain::{cleanup_from, cleanup_text, room_counting_text, Cleanup};
use snapall_core::health::format_bytes;
use snapall_core::services::{self, Job};
use snapall_core::QuotaMode;
use snapall_core::{AreaFacts, Facts, Health, Level, Remedy, Topic};

// ---- what the rows say --------------------------------------------------------------------------

/// The groups the rows fall into, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    NeedsALook,
    GoodToKnow,
    Space,
    Jobs,
}

impl Section {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Section::NeedsALook => "Needs a look",
            Section::GoodToKnow => "Good to know",
            Section::Space => "Space",
            Section::Jobs => "Background jobs",
        }
    }
}

/// One thing on the screen: what it is called, how it stands, and what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub section: Section,
    pub topic: Topic,
    pub level: Level,
    pub label: String,
    /// What it is, why it matters, what to do: shown beneath the list when the row is chosen.
    pub detail: String,
    /// What Enter does on this row, if anything.
    pub action: RowAction,
    /// The area this row is about, if it is one: what `U` (stop protecting) acts on.
    pub area: Option<snapall_core::AreaName>,
}

/// What Enter does on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowAction {
    Nothing,
    /// Something snapall can do about a finding.
    Fix(Remedy),
    /// Open the controls of a background job.
    ControlJob(Job),
    /// Switch counting the room snapshots take on (`true`) or off (`false`).
    CountRoom(bool),
    /// Change how many snapshots this area keeps, and when its oldest are removed.
    EditLimits(snapall_core::AreaName),
}

/// Everything the tab lists, in the order it is shown.
pub fn rows(data: &HealthData) -> Vec<Row> {
    let mut rows = finding_rows(&data.report);
    rows.extend(data.facts.areas.iter().map(space_row));
    rows.extend(room_row(&data.facts));
    rows.extend(job_rows(&data.facts));
    rows
}

pub(crate) fn finding_rows(report: &Health) -> Vec<Row> {
    report
        .findings
        .iter()
        .map(|finding| Row {
            section: if finding.level >= Level::Attention {
                Section::NeedsALook
            } else {
                Section::GoodToKnow
            },
            topic: finding.topic,
            level: finding.level,
            label: finding.headline.clone(),
            detail: finding.why.clone(),
            action: finding.remedy.map_or(RowAction::Nothing, RowAction::Fix),
            area: None,
        })
        .collect()
}

pub(crate) fn space_row(area: &AreaFacts) -> Row {
    let room = match area.measured_bytes {
        Some(bytes) => format!("{} held", format_bytes(bytes)),
        None => "room not measured".to_owned(),
    };
    let snapshots = format!(
        "{} snapshot{}",
        area.snapshots,
        if area.snapshots == 1 { "" } else { "s" }
    );
    let disk = area
        .disk
        .map_or(String::new(), |d| format!("  disk {}%", d.percent_used()));
    let level = match area.disk.map(|d| d.percent_used()) {
        Some(95..) => Level::Problem,
        Some(85..=94) => Level::Attention,
        _ => Level::Good,
    };
    Row {
        section: Section::Space,
        topic: Topic::Space,
        level,
        label: format!(
            "{} ({})  {snapshots}  {room}{disk}",
            area.title(),
            area.location
        ),
        detail: space_detail(area, &snapshots),
        // what it keeps can be changed, if snapper said what it keeps
        action: area
            .retention
            .as_ref()
            .and_then(snapall_core::Limits::from_retention)
            .map_or(RowAction::Nothing, |_| {
                RowAction::EditLimits(area.config.clone())
            }),
        area: Some(area.config.clone()),
    }
}

pub(crate) fn space_detail(area: &AreaFacts, snapshots: &str) -> String {
    let mut text = format!(
        "{} keeps its snapshots in {}. It has {snapshots}",
        area.title(),
        area.lives_in()
    );
    if let Some(oldest) = &area.oldest {
        text.push_str(&format!(", the oldest from {oldest}"));
    }
    text.push_str(". ");
    text.push_str(&match area.measured_bytes {
        Some(bytes) => format!(
            "Together they hold {}, room that nothing else uses. ",
            format_bytes(bytes)
        ),
        None => "The room they take is not measured (see the note about quotas). ".to_owned(),
    });
    if let Some(disk) = area.disk {
        text.push_str(&format!(
            "The disk it is on is {}% full. ",
            disk.percent_used()
        ));
    }
    if let Some(keep) = area.retention.as_ref().and_then(|r| r.keep) {
        text.push_str(&format!(
            "It keeps the newest {keep}{}.",
            minutes_note(area.retention.as_ref().and_then(|r| r.min_age_seconds))
        ));
    }
    text
}

/// ", and never cleans up one younger than 30 minutes".
pub(crate) fn minutes_note(seconds: Option<u64>) -> String {
    seconds.map_or(String::new(), |s| {
        format!(", and never cleans up one younger than {} minutes", s / 60)
    })
}

pub(crate) fn cleanup_state(facts: &Facts) -> Cleanup {
    let timer = facts.state_of(&services::CLEANUP);
    cleanup_from(timer.enabled.as_deref(), timer.active.as_deref())
}

/// How the disk counts the room snapshots take: one row, because it is one setting of the whole disk.
pub(crate) fn room_row(facts: &Facts) -> Option<Row> {
    let mode = facts.areas.first()?.quotas;
    let (level, word, action) = match mode {
        QuotaMode::Off => (Level::Note, "off", RowAction::CountRoom(true)),
        QuotaMode::Full => (Level::Good, "on", RowAction::CountRoom(false)),
        QuotaMode::Simple => (Level::Note, "on, the simple kind", RowAction::Nothing),
        QuotaMode::Unknown => (Level::Note, "could not be found out", RowAction::Nothing),
    };
    Some(Row {
        section: Section::Space,
        topic: Topic::Space,
        level,
        label: format!("Counting the room snapshots take: {word}"),
        detail: room_counting_text(mode),
        action,
        area: None,
    })
}

/// A job is listed when it is there to speak of: the cleanup job always (its absence matters), the
/// others when installed.
pub(crate) fn is_listed(facts: &Facts, job: &Job) -> bool {
    job.unit == services::CLEANUP.unit
        || facts.state_of(job).is_installed()
        || (job.unit == services::BOOT_MENU.unit && facts.boot_menu_plugin)
}

pub(crate) fn job_rows(facts: &Facts) -> Vec<Row> {
    services::ALL
        .iter()
        .filter(|job| is_listed(facts, job))
        .map(|job| job_row(facts, job))
        .collect()
}

pub(crate) fn job_row(facts: &Facts, job: &Job) -> Row {
    let state = facts.state_of(job);
    let plugin = job.unit == services::BOOT_MENU.unit && facts.boot_menu_plugin;
    let level = if job.unit == services::CLEANUP.unit {
        match cleanup_state(facts) {
            Cleanup::On => Level::Good,
            Cleanup::Off => Level::Attention,
            Cleanup::Unavailable => Level::Note,
        }
    } else if job.unit == services::BOOT_MENU.unit {
        if state.is_installed() && !state.is_running() && !plugin {
            Level::Attention
        } else {
            Level::Good
        }
    } else if state.is_running() {
        Level::Good
    } else {
        Level::Note
    };
    let word = if plugin {
        "kept up to date by a plugin"
    } else {
        state.words()
    };
    let schedule = facts.schedule_of(job);
    let details = facts.job(job).and_then(|j| j.details.as_ref());
    // the unit's own name and words first: they are what a person types and searches for
    let mut detail = match details.and_then(|d| d.description.as_deref()) {
        Some(description) => format!("{} (\"{description}\"). ", job.unit),
        None => format!("{}. ", job.unit),
    };
    if job.unit == services::CLEANUP.unit {
        detail.push_str(&cleanup_text(
            cleanup_state(facts),
            &schedule,
            &facts.cleanup_limits(),
        ));
    } else {
        detail.push_str(&format!("It {}.", job.purpose));
        if !schedule.is_empty() {
            detail.push_str(&format!(" It runs {}.", schedule.join(", ")));
        }
    }
    if level == Level::Attention && job.unit == services::BOOT_MENU.unit {
        if let Some(start) = job
            .commands()
            .into_iter()
            .find(|c| c.line.contains("enable --now"))
        {
            detail.push_str(&format!(" Start it with: {}", start.line));
        }
    }
    Row {
        section: Section::Jobs,
        topic: if job.unit == services::BOOT_MENU.unit {
            Topic::BootMenu
        } else {
            Topic::Cleanup
        },
        level,
        label: format!("{} ({}): {word}", job.title, job.unit),
        detail,
        action: RowAction::ControlJob(*job),
        area: None,
    }
}
