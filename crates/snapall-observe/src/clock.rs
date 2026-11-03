// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The time of a line of the log: the clock is read here, and the calendar that turns it into text is the core's.

use std::time::{SystemTime, UNIX_EPOCH};

/// `2026-10-23T05:43:12Z`. A time before 1970 (a broken clock) is shown as the epoch, because a
/// log line must always have a timestamp and must never fail to be written.
pub fn rfc3339_utc(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    snapall_core::vocabulary::calendar::rfc3339_utc(seconds)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(seconds: u64) -> String {
        rfc3339_utc(UNIX_EPOCH + Duration::from_secs(seconds))
    }

    #[test]
    fn known_instants_render_correctly() {
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(at(1_761_187_392), "2025-10-23T02:43:12Z");
    }

    #[test]
    fn leap_days_are_handled_including_the_century_rules() {
        assert_eq!(
            at(951_782_400),
            "2000-02-29T00:00:00Z",
            "2000 is a leap year"
        );
        assert_eq!(at(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(at(1_709_251_200), "2024-03-01T00:00:00Z");
        // 2100 is not a leap year: the day after Feb 28th is Mar 1st.
        assert_eq!(at(4_107_456_000), "2100-02-28T00:00:00Z");
        assert_eq!(at(4_107_542_400), "2100-03-01T00:00:00Z");
    }

    #[test]
    fn the_last_second_of_a_year_and_the_first_of_the_next() {
        assert_eq!(at(1_704_067_199), "2023-12-31T23:59:59Z");
        assert_eq!(at(1_704_067_200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn a_clock_before_the_epoch_does_not_break_logging() {
        assert_eq!(
            rfc3339_utc(UNIX_EPOCH - Duration::from_secs(5)),
            "1970-01-01T00:00:00Z"
        );
    }
}
