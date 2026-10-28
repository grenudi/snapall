// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How many snapshots an area keeps, and the options that say so.

/// How an area's snapshots are kept and cleaned up, as snapper has it set. Fields snapper does not
/// report, or reports in a form snapall does not understand, are `None`: nothing is guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retention {
    /// The newest snapshots kept (`NUMBER_LIMIT`).
    pub keep: Option<u32>,
    /// Snapshots marked important are kept this many (`NUMBER_LIMIT_IMPORTANT`).
    pub keep_important: Option<u32>,
    /// A snapshot younger than this many seconds is never cleaned up (`NUMBER_MIN_AGE`).
    pub min_age_seconds: Option<u64>,
    /// Whether cleaning up numbered snapshots is switched on for this area (`NUMBER_CLEANUP`).
    pub cleanup: bool,
    /// Whether snapper takes snapshots of this area by itself on a schedule (`TIMELINE_CREATE`).
    pub on_a_schedule: bool,
    /// The share of the disk the snapshots may fill before snapper cleans up (`SPACE_LIMIT`), in
    /// thousandths (500 is half), which only acts when Btrfs quotas are on.
    pub space_limit_permille: Option<u16>,
    /// Whether Btrfs quotas are set up for the area (`QGROUP` is set), which is what makes the space
    /// each snapshot holds knowable.
    pub quotas: bool,
}

impl Retention {
    /// From snapper's settings, as the `key -> value` text it reports them in.
    pub fn from_settings(settings: &std::collections::BTreeMap<String, String>) -> Self {
        let text = |key: &str| {
            settings
                .get(key)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
        };
        let number = |key: &str| text(key).and_then(|v| v.parse::<u64>().ok());
        Self {
            keep: number("NUMBER_LIMIT").and_then(|n| u32::try_from(n).ok()),
            keep_important: number("NUMBER_LIMIT_IMPORTANT").and_then(|n| u32::try_from(n).ok()),
            min_age_seconds: number("NUMBER_MIN_AGE"),
            cleanup: text("NUMBER_CLEANUP") == Some("yes"),
            on_a_schedule: text("TIMELINE_CREATE") == Some("yes"),
            space_limit_permille: text("SPACE_LIMIT")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|share| (0.0..=1.0).contains(share))
                .map(|share| (share * 1000.0).round() as u16),
            quotas: text("QGROUP").is_some(),
        }
    }
}

/// A setting of an area that snapall chooses for the areas it creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaOption {
    /// Whether snapper takes snapshots by itself on a schedule (hourly, daily, ...). snapall turns
    /// it off for what it sets up: snapshots happen when you ask, so nothing piles up on its own.
    TimelineCreate(bool),
    /// How many of the snapshots that cleanup looks after are kept (`NUMBER_LIMIT`): the newest this
    /// many stay, the older ones are removed at the next cleanup.
    KeepNewest(u32),
    /// Nothing younger than this many seconds is ever removed by cleanup (`NUMBER_MIN_AGE`).
    NeverRemoveYoungerThan(u64),
    /// Whether cleanup removes anything of this area at all (`NUMBER_CLEANUP`).
    CleanupOfSnapshots(bool),
}

impl AreaOption {
    /// What the setting means, for the audit log.
    pub fn describe(self) -> String {
        match self {
            AreaOption::TimelineCreate(on) => {
                format!("snapshots on a schedule: {}", if on { "on" } else { "off" })
            }
            AreaOption::KeepNewest(count) => format!("keep the newest {count}"),
            AreaOption::NeverRemoveYoungerThan(seconds) => {
                format!("never remove one younger than {seconds} seconds")
            }
            AreaOption::CleanupOfSnapshots(on) => {
                format!("cleanup of snapshots: {}", if on { "on" } else { "off" })
            }
        }
    }
}
