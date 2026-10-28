// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How many snapshots an area keeps, and when the oldest go: the settings of automatic cleanup, and what
//! changing them would do.
//!
//! Lowering how many are kept is the one change here that destroys something: at the next cleanup the
//! older snapshots are removed, and a removed snapshot cannot be brought back. So before a change is made
//! it is *previewed*: this module says how many snapshots would go, in the order cleanup would take them,
//! so the person decides knowing it, not finding out after.

use crate::vocabulary::calendar::seconds_from_snapshot_date;
use crate::vocabulary::domain::{AreaOption, Retention, SnapshotInfo};
use crate::vocabulary::explain::seconds_words;

/// The longest a snapshot can be protected from cleanup: a year.
pub const LONGEST_PROTECTION_SECONDS: u64 = 365 * 24 * 3600;

/// The most that can be kept: far beyond what any disk holds, and a guard against a typing slip.
pub const MOST_KEPT: u32 = 10_000;

/// An area's cleanup settings, as a person sets them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How many of the snapshots that cleanup looks after are kept.
    pub keep: u32,
    /// Nothing younger than this is removed, however many there are.
    pub min_age_seconds: u64,
    /// Whether cleanup removes anything of this area at all.
    pub cleanup: bool,
}

impl Limits {
    /// What snapper has now; `None` if it did not say how many are kept (then there is nothing to edit).
    pub fn from_retention(retention: &Retention) -> Option<Limits> {
        Some(Limits {
            keep: retention.keep?,
            min_age_seconds: retention.min_age_seconds.unwrap_or(0),
            cleanup: retention.cleanup,
        })
    }

    /// The settings in words: what they keep, and the age under which nothing goes.
    pub fn describe(&self) -> String {
        format!(
            "keeps the newest {}, never removes one younger than {} minutes, cleanup {}",
            self.keep,
            self.min_age_seconds / 60,
            if self.cleanup { "on" } else { "off" }
        )
    }

    /// Why these cannot be set, in a sentence, if they cannot.
    pub fn problem(&self) -> Option<String> {
        if self.keep == 0 {
            return Some("Keep at least one: keeping none would let cleanup remove every snapshot (to stop cleanup, switch it off instead)".to_owned());
        }
        if self.keep > MOST_KEPT {
            return Some(format!("Keep at most {MOST_KEPT}"));
        }
        if self.min_age_seconds > LONGEST_PROTECTION_SECONDS {
            return Some("Protect snapshots from cleanup for at most a year".to_owned());
        }
        None
    }

    /// The settings to change to get from `before` to `self`, and only those.
    pub fn changes_from(&self, before: &Limits) -> Vec<AreaOption> {
        let mut options = Vec::new();
        if self.keep != before.keep {
            options.push(AreaOption::KeepNewest(self.keep));
        }
        if self.min_age_seconds != before.min_age_seconds {
            options.push(AreaOption::NeverRemoveYoungerThan(self.min_age_seconds));
        }
        if self.cleanup != before.cleanup {
            options.push(AreaOption::CleanupOfSnapshots(self.cleanup));
        }
        options
    }
}

/// What the next cleanup would do with these settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preview {
    /// How many snapshots cleanup looks after in this area.
    pub looked_after: usize,
    /// How many of them it would remove.
    pub would_remove: usize,
    /// How many beyond the limit it would leave only because they are too young to remove.
    pub spared_by_age: usize,
    /// Every age was known; if not, `would_remove` is the most it could be.
    pub exact: bool,
}

/// What the next cleanup would remove: of the snapshots it looks after (those marked for the `number`
/// kind of cleanup), the oldest ones beyond `limits.keep`, except those younger than the protection age.
/// `now` is the machine's clock in the same terms as the snapshots' dates; without it an age is unknown
/// and counts as old enough, so the answer is then a ceiling, and says so.
pub fn preview(snapshots: &[SnapshotInfo], limits: &Limits, now: Option<i64>) -> Preview {
    let mut looked_after: Vec<&SnapshotInfo> = snapshots
        .iter()
        .filter(|snapshot| snapshot.cleanup.as_deref() == Some("number"))
        .collect();
    looked_after.sort_by_key(|snapshot| snapshot.id);
    let total = looked_after.len();
    if !limits.cleanup {
        return Preview {
            looked_after: total,
            would_remove: 0,
            spared_by_age: 0,
            exact: true,
        };
    }
    let beyond = total.saturating_sub(usize::try_from(limits.keep).unwrap_or(usize::MAX));
    let mut exact = true;
    let mut spared = 0;
    for snapshot in looked_after.iter().take(beyond) {
        let age = match (
            now,
            snapshot
                .date
                .as_deref()
                .and_then(seconds_from_snapshot_date),
        ) {
            (Some(now), Some(then)) => u64::try_from(now - then).ok(),
            _ => None,
        };
        match age {
            Some(age) if age < limits.min_age_seconds => spared += 1,
            Some(_) => {}
            None => exact = false,
        }
    }
    Preview {
        looked_after: total,
        would_remove: beyond - spared,
        spared_by_age: spared,
        exact,
    }
}

/// The preview in words, for the person deciding.
pub fn preview_words(preview: &Preview, limits: &Limits) -> String {
    if !limits.cleanup {
        return "Cleanup is switched off for this area, so nothing will be removed however few are kept.".to_owned();
    }
    if preview.would_remove == 0 {
        let spared = if preview.spared_by_age > 0 {
            format!(
                " ({} more than the limit are kept only because they are younger than {})",
                preview.spared_by_age,
                seconds_words(limits.min_age_seconds)
            )
        } else {
            String::new()
        };
        return format!(
            "Nothing would be removed: cleanup looks after {} snapshot{} and the limit is {}{spared}.",
            preview.looked_after,
            if preview.looked_after == 1 { "" } else { "s" },
            limits.keep
        );
    }
    let at_most = if preview.exact { "" } else { "up to " };
    let spared = if preview.spared_by_age > 0 {
        format!(
            "; {} more are kept for now because they are younger than {}",
            preview.spared_by_age,
            seconds_words(limits.min_age_seconds)
        )
    } else {
        String::new()
    };
    format!(
        "At the next cleanup {at_most}{} of the {} snapshots that cleanup looks after would be removed, the oldest first{spared}. A removed snapshot cannot be brought back.",
        preview.would_remove, preview.looked_after
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocabulary::domain::{SnapshotId, SnapshotKind};

    /// A snapshot of cleanup kind `kind`, number `id`, taken `ago` seconds before `NOW`.
    const NOW: i64 = 1_000_000;

    fn dated(id: u64, cleanup: Option<&str>, ago: i64) -> SnapshotInfo {
        let seconds = NOW - ago;
        // a date snapper would write for that moment, in the one calendar this module reads
        let date = crate::vocabulary::calendar::snapshot_date_from_seconds(seconds);
        SnapshotInfo {
            id: SnapshotId::new(id).unwrap(),
            kind: SnapshotKind::Single,
            date: Some(date),
            user: "root".into(),
            cleanup: cleanup.map(str::to_owned),
            description: String::new(),
            point: None,
            used_bytes: None,
        }
    }

    fn limits(keep: u32, min_age_seconds: u64) -> Limits {
        Limits {
            keep,
            min_age_seconds,
            cleanup: true,
        }
    }

    #[test]
    fn what_cannot_be_set_is_refused_with_a_sentence() {
        assert!(limits(0, 0)
            .problem()
            .unwrap()
            .contains("Keep at least one"));
        assert!(limits(10_001, 0)
            .problem()
            .unwrap()
            .contains("at most 10000"));
        assert!(limits(5, LONGEST_PROTECTION_SECONDS + 1)
            .problem()
            .unwrap()
            .contains("at most a year"));
        assert_eq!(limits(50, 1800).problem(), None);
        assert_eq!(limits(1, 0).problem(), None);
    }

    #[test]
    fn only_what_differs_is_changed() {
        let before = limits(50, 1800);
        assert!(before.changes_from(&before).is_empty());
        assert_eq!(
            limits(20, 1800).changes_from(&before),
            [AreaOption::KeepNewest(20)]
        );
        assert_eq!(
            Limits {
                keep: 50,
                min_age_seconds: 3600,
                cleanup: false
            }
            .changes_from(&before),
            [
                AreaOption::NeverRemoveYoungerThan(3600),
                AreaOption::CleanupOfSnapshots(false)
            ]
        );
    }

    #[test]
    fn the_oldest_beyond_the_limit_are_what_would_go_and_only_those_cleanup_looks_after() {
        // ten numbered snapshots, one a day older than the last, and three that cleanup never touches
        let mut all: Vec<SnapshotInfo> = (1..=10)
            .map(|id| dated(id, Some("number"), (11 - id as i64) * 86_400))
            .collect();
        all.extend([
            dated(11, None, 100),
            dated(12, Some("timeline"), 100),
            dated(13, Some("number"), 50),
        ]);
        let got = preview(&all, &limits(6, 1800), Some(NOW));
        // eleven are looked after (ten, and #13); six are kept; the five oldest would go
        assert_eq!(
            (
                got.looked_after,
                got.would_remove,
                got.spared_by_age,
                got.exact
            ),
            (11, 5, 0, true)
        );
        assert_eq!(
            preview(&all, &limits(11, 1800), Some(NOW)).would_remove,
            0,
            "exactly at the limit nothing goes"
        );
        assert_eq!(preview(&all, &limits(50, 1800), Some(NOW)).would_remove, 0);
    }

    #[test]
    fn nothing_younger_than_the_protection_age_goes_even_beyond_the_limit() {
        // three snapshots minutes old, limit one: two are beyond it, but they are younger than an hour
        let all: Vec<SnapshotInfo> = (1..=3)
            .map(|id| dated(id, Some("number"), 600 - id as i64))
            .collect();
        let got = preview(&all, &limits(1, 3600), Some(NOW));
        assert_eq!((got.would_remove, got.spared_by_age), (0, 2));
        assert_eq!(
            preview(&all, &limits(1, 60), Some(NOW)).would_remove,
            2,
            "old enough once the age is lower"
        );
    }

    #[test]
    fn with_cleanup_off_nothing_goes_whatever_the_limit() {
        let all: Vec<SnapshotInfo> = (1..=5)
            .map(|id| dated(id, Some("number"), 86_400))
            .collect();
        let off = Limits {
            keep: 1,
            min_age_seconds: 0,
            cleanup: false,
        };
        assert_eq!(preview(&all, &off, Some(NOW)).would_remove, 0);
        assert!(preview_words(&preview(&all, &off, Some(NOW)), &off).contains("switched off"));
    }

    #[test]
    fn when_the_time_is_not_known_the_answer_is_a_ceiling_and_says_so() {
        let all: Vec<SnapshotInfo> = (1..=5).map(|id| dated(id, Some("number"), 60)).collect();
        let got = preview(&all, &limits(2, 3600), None);
        assert!(!got.exact && got.would_remove == 3);
        assert!(preview_words(&got, &limits(2, 3600)).contains("up to 3 of the 5"));
    }

    #[test]
    fn the_words_say_how_many_which_first_and_that_it_cannot_be_undone() {
        let all: Vec<SnapshotInfo> = (1..=8)
            .map(|id| dated(id, Some("number"), 86_400 * id as i64))
            .collect();
        let words = preview_words(
            &preview(&all, &limits(5, 1800), Some(NOW)),
            &limits(5, 1800),
        );
        assert!(
            words.contains(
                "3 of the 8 snapshots that cleanup looks after would be removed, the oldest first"
            ),
            "{words}"
        );
        assert!(words.contains("cannot be brought back"), "{words}");
        let none = preview_words(
            &preview(&all, &limits(8, 1800), Some(NOW)),
            &limits(8, 1800),
        );
        assert!(
            none.starts_with(
                "Nothing would be removed: cleanup looks after 8 snapshots and the limit is 8"
            ),
            "{none}"
        );
        // and the age protection is explained when it is what spares them
        let young: Vec<SnapshotInfo> = (1..=4).map(|id| dated(id, Some("number"), 100)).collect();
        let spared = preview_words(
            &preview(&young, &limits(1, 3600), Some(NOW)),
            &limits(1, 3600),
        );
        assert!(
            spared.contains(
                "3 more than the limit are kept only because they are younger than 1 hour"
            ),
            "{spared}"
        );
    }
}
