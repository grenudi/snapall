// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The chips that say how things stand at a glance.

use super::*;
use crate::explain::Cleanup;

/// The items of the status bar, in the order they are read.
pub(super) fn chips(facts: &Facts, findings: &[Finding]) -> Vec<Chip> {
    let mut chips = vec![snapshot_count(facts), snapshot_space(facts)];
    chips.extend(disk_chip(facts));
    chips.push(cleanup_chip(facts));
    chips.extend(attention_chip(findings));
    chips
}

pub(super) fn snapshot_count(facts: &Facts) -> Chip {
    let total: usize = facts.areas.iter().map(|a| a.snapshots).sum();
    Chip {
        label: format!("{total} snapshot{}", if total == 1 { "" } else { "s" }),
        level: Level::Good,
        topic: Topic::Snapshots,
    }
}

pub(super) fn snapshot_space(facts: &Facts) -> Chip {
    let all: Option<u64> = facts.areas.iter().map(|a| a.measured_bytes).sum();
    match all.filter(|_| !facts.areas.is_empty()) {
        Some(bytes) => Chip {
            label: format_bytes(bytes),
            level: Level::Good,
            topic: Topic::Space,
        },
        None => Chip {
            label: "size not measured".to_owned(),
            level: Level::Note,
            topic: Topic::Space,
        },
    }
}

pub(super) fn disk_chip(facts: &Facts) -> Option<Chip> {
    let (_, disk) = fullest_disk(facts)?;
    let level = match disk.percent_used() {
        95.. => Level::Problem,
        85..=94 => Level::Attention,
        _ => Level::Good,
    };
    Some(Chip {
        label: format!("disk {}% full", disk.percent_used()),
        level,
        topic: Topic::Space,
    })
}

pub(super) fn cleanup_chip(facts: &Facts) -> Chip {
    let (label, level) = match cleanup_state(facts) {
        Cleanup::On => ("cleanup on", Level::Good),
        Cleanup::Off => ("cleanup off", Level::Attention),
        Cleanup::Unavailable => ("cleanup unknown", Level::Note),
    };
    Chip {
        label: label.to_owned(),
        level,
        topic: Topic::Cleanup,
    }
}

/// How many things need attention, when any do; a click goes to the most urgent.
pub(super) fn attention_chip(findings: &[Finding]) -> Option<Chip> {
    let urgent: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.level >= Level::Attention)
        .collect();
    let first = urgent.first()?;
    Some(Chip {
        label: format!("{} to look at", urgent.len()),
        level: first.level,
        topic: first.topic,
    })
}
