// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a person is told about cleanup and about how room is counted.

use crate::ports::machine::QuotaMode;

/// How automatic cleanup stands on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cleanup {
    /// The job that removes the oldest snapshots is switched on and running.
    On,
    /// It exists and is switched off: snapshots would pile up. It can be switched on.
    Off,
    /// There is nothing to switch on (not installed, or this computer has no such jobs), or it could
    /// not be told.
    Unavailable,
}

/// The state of automatic cleanup from what `systemctl is-enabled` and `systemctl is-active` printed
/// for the cleanup timer (`None` if they could not be asked). It is **on** only if it is enabled and
/// running: a timer that is enabled but not running (a container, a system not started by systemd, a
/// timer that failed) is not presented as doing its job.
pub fn cleanup_from(is_enabled: Option<&str>, is_active: Option<&str>) -> Cleanup {
    let enabled = matches!(
        is_enabled.map(str::trim),
        Some("enabled" | "enabled-runtime" | "alias" | "static" | "indirect" | "generated")
    );
    match (is_enabled.map(str::trim), is_active.map(str::trim)) {
        (Some("disabled"), _) => Cleanup::Off,
        (_, Some("active")) if enabled => Cleanup::On,
        _ => Cleanup::Unavailable,
    }
}

/// How one area's cleanup is set, as snapper has it: the limit and the age under which nothing goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limit {
    /// What a person calls the area: `Home`.
    pub area: String,
    pub keep: Option<u32>,
    pub min_age_seconds: Option<u64>,
}

/// `1800` as `30 minutes`, `3600` as `1 hour`, `90` as `90 seconds`.
pub fn seconds_words(seconds: u64) -> String {
    let (count, unit) = match seconds {
        s if s >= 86_400 && s % 86_400 == 0 => (s / 86_400, "day"),
        s if s >= 3_600 && s % 3_600 == 0 => (s / 3_600, "hour"),
        s if s >= 60 && s % 60 == 0 => (s / 60, "minute"),
        s => (s, "second"),
    };
    format!("{count} {unit}{}", if count == 1 { "" } else { "s" })
}

/// When the job runs, as its unit file says; if that is not known, it says so rather than guess.
pub(super) fn when_it_runs(schedule: &[String]) -> String {
    if schedule.is_empty() {
        "on the schedule your system gives it".to_owned()
    } else {
        schedule.join(", ")
    }
}

/// What each area keeps and protects from cleanup, from its real settings; nothing if none is known.
pub(super) fn limits_sentence(limits: &[Limit]) -> String {
    let parts: Vec<String> = limits
        .iter()
        .filter_map(|limit| {
            let keep = limit.keep?;
            Some(match limit.min_age_seconds {
                Some(age) => format!(
                    "{} keeps the newest {keep} and never removes one younger than {}",
                    limit.area,
                    seconds_words(age)
                ),
                None => format!("{} keeps the newest {keep}", limit.area),
            })
        })
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{}.", parts.join("; "))
    }
}

/// What automatic cleanup means for the person, on this computer, from its real schedule and limits.
pub fn cleanup_text(state: Cleanup, schedule: &[String], limits: &[Limit]) -> String {
    let limits = limits_sentence(limits);
    let when = when_it_runs(schedule);
    let text = match state {
        Cleanup::On => format!(
            "Old snapshots are removed automatically. A background job runs {when} and, in each area that has more than its limit, removes the oldest. Nothing piles up."
        ),
        Cleanup::Off => format!(
            "Old snapshots are NOT removed automatically on this computer. The background job that does it (it runs {when}) is switched off, so snapshots would pile up and use more and more space."
        ),
        Cleanup::Unavailable => "snapall could not find out whether old snapshots are removed automatically on this computer. If they are not, they pile up: you can delete them yourself in the snapall window.".to_owned(),
    };
    if limits.is_empty() || state == Cleanup::Unavailable {
        text
    } else {
        format!("{text} {limits}")
    }
}

/// What switching cleanup on does, to ask before doing it.
pub fn cleanup_offer(schedule: &[String], limits: &[Limit]) -> String {
    let limits = limits_sentence(limits);
    let when = when_it_runs(schedule);
    let mut text = format!(
        "Switch it on? This starts the small background job that runs {when} and removes the oldest snapshots of any area that has more than its limit."
    );
    if !limits.is_empty() {
        text.push(' ');
        text.push_str(&limits);
    }
    text.push_str(" It touches nothing else.");
    text
}

/// What counting the room snapshots take means: what it gives, what it costs, and whether it is worth it,
/// for how this disk counts it now. The filesystem's own name for it, quotas, is given once so that it can
/// be looked up.
pub fn room_counting_text(mode: QuotaMode) -> String {
    match mode {
        QuotaMode::Off => "The disk is not counting how much room each snapshot holds. snapall can show how many snapshots there are and how full the disk is, but not what a snapshot costs. Switching counting on (it is called quotas, should you want to look it up) shows that, and lets cleanup act on room as well as on numbers. The price: the disk then has to keep a tally up to date, which is known to slow some operations down, deleting snapshots most of all, and the tally is built once when it is switched on, which takes a while on a big disk. It applies to the whole disk, not to one area, and it can be switched off again. With few snapshots, and no need to see their size, leaving it off is the better choice.".to_owned(),
        QuotaMode::Full => "The disk counts room fully (it is called quotas, should you want to look it up), so what each snapshot alone holds is shown, and cleanup can act on room as well as on numbers. The price is a tally the disk keeps up to date, which is known to slow some operations down, deleting snapshots most of all. It applies to the whole disk. Switching it off removes the tally and the figures; nothing else changes.".to_owned(),
        QuotaMode::Simple => "The disk counts room the simple way, a newer and cheaper kind. It charges room to whichever area first wrote the data, so what a snapshot holds on to cannot be told from it, and snapall shows no figure per snapshot. Counting fully would show it, at the cost of a slower disk when snapshots are deleted; switching from one kind to the other is not something snapall can do in place.".to_owned(),
        QuotaMode::Unknown => "snapall could not find out how this disk counts room.".to_owned(),
    }
}
