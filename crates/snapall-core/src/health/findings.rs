// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding what is wrong, one check at a time, and the verdict they add up to.

use super::*;
use crate::keeping::jobs::{self};
use crate::ports::machine::{DiskUsage, QuotaMode};
use crate::vocabulary::calendar::seconds_from_snapshot_date;
use crate::vocabulary::explain::{cleanup_from, cleanup_text, room_counting_text, Cleanup};

/// Everything worth saying about how protection stands, most urgent first.
pub fn assess(facts: &Facts) -> Health {
    let mut findings = Vec::new();
    findings.extend(the_disk(facts));
    findings.extend(automatic_cleanup(facts));
    findings.extend(areas_over_their_limit(facts));
    findings.extend(space_not_measured(facts));
    findings.extend(snapshots_made_on_a_schedule(facts));
    findings.extend(areas_gone_quiet(facts));
    findings.extend(the_boot_menu(facts));
    findings.sort_by_key(|finding| std::cmp::Reverse(finding.level)); // stable: ties stay in story order
    Health {
        verdict: verdict(&findings),
        chips: chips(facts, &findings),
        findings,
    }
}

/// The fullest disk that holds an area, with the area.
pub(super) fn fullest_disk(facts: &Facts) -> Option<(&AreaFacts, DiskUsage)> {
    facts
        .areas
        .iter()
        .filter_map(|area| Some((area, area.disk?)))
        .max_by_key(|(_, disk)| disk.percent_used())
}

pub(super) fn the_disk(facts: &Facts) -> Option<Finding> {
    let (area, disk) = fullest_disk(facts)?;
    let percent = disk.percent_used();
    let level = match percent {
        95.. => Level::Problem,
        85..=94 => Level::Attention,
        _ => return None,
    };
    Some(Finding {
        level,
        topic: Topic::Space,
        headline: format!("The disk holding {} is {percent}% full", area.title()),
        why: "A disk that is nearly full gets slow and can stop accepting changes. Snapshots keep old versions of files, so they hold space that only they use: deleting old snapshots frees it, and automatic cleanup does that for you while it runs.".to_owned(),
        remedy: None,
    })
}

pub(super) fn cleanup_state(facts: &Facts) -> Cleanup {
    let timer = facts.state_of(&jobs::CLEANUP);
    cleanup_from(timer.enabled.as_deref(), timer.active.as_deref())
}

pub(super) fn automatic_cleanup(facts: &Facts) -> Option<Finding> {
    let state = cleanup_state(facts);
    let (level, headline, remedy) = match state {
        Cleanup::On => return None,
        Cleanup::Off => (
            Level::Attention,
            "Old snapshots are not cleaned up automatically",
            Some(Remedy::SwitchOnCleanup),
        ),
        Cleanup::Unavailable => (
            Level::Note,
            "snapall could not find out whether old snapshots are cleaned up",
            None,
        ),
    };
    Some(Finding {
        level,
        topic: Topic::Cleanup,
        headline: headline.to_owned(),
        why: cleanup_text(
            state,
            &facts.schedule_of(&jobs::CLEANUP),
            &facts.cleanup_limits(),
        ),
        remedy,
    })
}

/// Areas that hold more snapshots than their limit while nothing is cleaning them up.
pub(super) fn areas_over_their_limit(facts: &Facts) -> Option<Finding> {
    if cleanup_state(facts) == Cleanup::On {
        return None;
    }
    let over: Vec<String> = facts
        .areas
        .iter()
        .filter_map(|area| {
            let keep = area.retention.as_ref()?.keep?;
            (area.snapshots > usize::try_from(keep).ok()?).then(|| {
                format!(
                    "{} has {} (its limit is {keep})",
                    area.title(),
                    area.snapshots
                )
            })
        })
        .collect();
    (!over.is_empty()).then(|| Finding {
        level: Level::Attention,
        topic: Topic::Cleanup,
        headline: format!("More snapshots than the limit: {}", over.join("; ")),
        why: "Each area keeps its newest snapshots up to a limit and removes the older ones, but only when the cleanup job runs. Until it does, they pile up and take more and more room.".to_owned(),
        remedy: Some(Remedy::SwitchOnCleanup),
    })
}

pub(super) fn space_not_measured(facts: &Facts) -> Option<Finding> {
    let unmeasured = facts
        .areas
        .iter()
        .find(|area| area.measured_bytes.is_none())?;
    let mode = unmeasured.quotas;
    Some(Finding {
        level: Level::Note,
        topic: Topic::Space,
        headline: "The room each snapshot takes is not measured".to_owned(),
        why: room_counting_text(mode),
        // only the full kind can answer the question, so only that is offered
        remedy: (mode == QuotaMode::Off).then_some(Remedy::CountRoom),
    })
}

pub(super) fn snapshots_made_on_a_schedule(facts: &Facts) -> Option<Finding> {
    let names: Vec<String> = facts
        .areas
        .iter()
        .filter(|area| area.retention.as_ref().is_some_and(|r| r.on_a_schedule))
        .map(AreaFacts::title)
        .collect();
    (!names.is_empty()).then(|| Finding {
        level: Level::Note,
        topic: Topic::Snapshots,
        headline: format!("Snapshots are also made on a schedule in {}", names.join(", ")),
        why: "On top of the ones you ask for, the snapshot tool's own timer makes more, and cleanup removes the old ones. When snapall sets an area up it turns this off, so only your snapshots exist.".to_owned(),
        remedy: None,
    })
}

/// Areas with no snapshot at all, or none for a month.
pub(super) fn areas_gone_quiet(facts: &Facts) -> Vec<Finding> {
    let now = facts.now.as_deref().and_then(seconds_from_snapshot_date);
    facts
        .areas
        .iter()
        .filter_map(|area| {
            let Some(newest) = area.newest.as_deref().and_then(seconds_from_snapshot_date) else {
                return (area.snapshots == 0).then(|| Finding {
                    level: Level::Note,
                    topic: Topic::Snapshots,
                    headline: format!("{} has no snapshots yet", area.title()),
                    why: "Nothing can be put back until one is taken. Press a, or run sudo snapall, to take the first.".to_owned(),
                    remedy: None,
                });
            };
            let days = (now? - newest) / 86_400;
            (days >= 30).then(|| Finding {
                level: Level::Note,
                topic: Topic::Snapshots,
                headline: format!("The newest snapshot of {} is {days} days old", area.title()),
                why: "A snapshot only protects what was there when it was taken: changes since then cannot be undone. Take one before your next change.".to_owned(),
                remedy: None,
            })
        })
        .collect()
}

/// The tool that puts snapshots into the boot menu is installed but not working.
pub(super) fn the_boot_menu(facts: &Facts) -> Option<Finding> {
    let sync = &facts.state_of(&jobs::BOOT_MENU);
    let installed_but_idle = sync.is_installed() && !sync.is_running() && !facts.boot_menu_plugin;
    installed_but_idle.then(|| Finding {
        level: Level::Attention,
        topic: Topic::BootMenu,
        headline: "New snapshots are not reaching the boot menu".to_owned(),
        why: "The tool that adds your snapshots to the boot menu (limine-snapper-sync) is installed but not running, so you could not pick a recent snapshot when starting the computer. Start it with: systemctl enable --now limine-snapper-sync.service".to_owned(),
        remedy: None,
    })
}

pub(super) fn verdict(findings: &[Finding]) -> Verdict {
    let worst = findings
        .iter()
        .map(|f| f.level)
        .max()
        .unwrap_or(Level::Good);
    let to_do = findings
        .iter()
        .filter(|f| f.level >= Level::Attention)
        .count();
    let text = match (worst, to_do) {
        (Level::Problem, _) => "Needs attention now".to_owned(),
        (_, 0) => "All good".to_owned(),
        (_, 1) => "1 thing needs attention".to_owned(),
        (_, n) => format!("{n} things need attention"),
    };
    Verdict {
        level: if to_do == 0 { Level::Good } else { worst },
        text,
    }
}
