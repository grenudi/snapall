// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dates, without a clock: turning seconds into a name for a restore point, and snapper's own date
//! text into seconds so two snapshots can be compared. The core has no clock; the shell reads one and
//! hands the number in.

/// Days since 1970-01-01 of a civil date (proleptic Gregorian). Howard Hinnant's algorithm.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The civil date of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// `20261025T181500Z` for 2026-10-25 18:15:00 UTC.
pub fn compact_utc(seconds: u64) -> String {
    let seconds = i64::try_from(seconds).unwrap_or(i64::MAX / 2);
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let in_day = seconds.rem_euclid(86_400);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        in_day / 3600,
        in_day % 3600 / 60,
        in_day % 60
    )
}

/// Seconds for snapper's `2026-10-25 18:15:00`. The zone is not known, and does not matter: only
/// the difference between two such dates is ever used.
pub fn seconds_from_snapshot_date(text: &str) -> Option<i64> {
    let (date, time) = text.trim().split_once(' ')?;
    let mut d = date.split('-');
    let (year, month, day) = (
        d.next()?.parse::<i64>().ok()?,
        d.next()?.parse::<i64>().ok()?,
        d.next()?.parse::<i64>().ok()?,
    );
    let mut t = time.split(':');
    let (hour, minute, second) = (
        t.next()?.parse::<i64>().ok()?,
        t.next()?.parse::<i64>().ok()?,
        t.next()?.parse::<i64>().ok()?,
    );
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && (0..24).contains(&hour)
        && (0..60).contains(&minute)
        && (0..61).contains(&second)
        && d.next().is_none()
        && t.next().is_none();
    valid.then(|| days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second)
}

/// Snapper's `2026-10-25 18:15:00` for those seconds: the way back from [`seconds_from_snapshot_date`].
pub fn snapshot_date_from_seconds(seconds: i64) -> String {
    let (days, in_day) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        in_day / 3600,
        in_day % 3600 / 60,
        in_day % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_survives_the_trip_to_seconds_and_back() {
        for text in [
            "2026-10-25 18:15:00",
            "1970-01-01 00:00:00",
            "2000-02-29 23:59:59",
            "2038-01-19 03:14:07",
        ] {
            let seconds = seconds_from_snapshot_date(text).unwrap();
            assert_eq!(snapshot_date_from_seconds(seconds), text);
        }
    }

    #[test]
    fn seconds_become_a_name_across_years_leaps_and_the_epoch() {
        assert_eq!(compact_utc(0), "19700101T000000Z");
        assert_eq!(compact_utc(1_700_000_000), "20231114T221320Z");
        assert_eq!(compact_utc(951_782_400), "20000229T000000Z", "a leap day");
        assert_eq!(compact_utc(1_782_398_100), "20260625T143500Z");
        assert_eq!(compact_utc(4_102_444_799), "20991231T235959Z");
    }

    #[test]
    fn snappers_dates_become_seconds_that_compare_and_bad_ones_are_refused() {
        let a = seconds_from_snapshot_date("2026-10-25 18:26:52").unwrap();
        let b = seconds_from_snapshot_date("2026-10-25 18:27:10").unwrap();
        assert_eq!(b - a, 18);
        let midnight = seconds_from_snapshot_date("2026-10-26 00:00:01").unwrap();
        assert_eq!(
            midnight - seconds_from_snapshot_date("2026-10-25 23:59:59").unwrap(),
            2
        );
        for bad in [
            "",
            "2026-10-25",
            "2026-13-25 10:00:00",
            "2026-10-25 25:00:00",
            "x y",
            "2026-10-25 10:00",
        ] {
            assert_eq!(seconds_from_snapshot_date(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_two_directions_agree() {
        for seconds in [0_u64, 86_399, 86_400, 1_700_000_000, 4_000_000_000] {
            let name = compact_utc(seconds);
            let spaced = format!(
                "{}-{}-{} {}:{}:{}",
                &name[0..4],
                &name[4..6],
                &name[6..8],
                &name[9..11],
                &name[11..13],
                &name[13..15]
            );
            assert_eq!(
                seconds_from_snapshot_date(&spaced),
                Some(seconds as i64),
                "{name}"
            );
        }
    }
}
