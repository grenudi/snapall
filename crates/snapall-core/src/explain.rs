// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall tells a person about what it does, in words that assume nothing: no snapshots, no
//! filesystems, no snapper. Every screen and command that explains something takes its text from
//! here, so they say the same thing, and one test keeps the jargon out of all of it.
//!
//! The vocabulary: a **snapshot** is a saved moment; a **protected area** is a part of the disk that
//! snapshots are kept for (what snapper calls a config, and Btrfs a subvolume: words nobody should
//! need).

use crate::coverage::{describe, Area, Info, Worth};
use crate::system::QuotaMode;
use crate::usecase::SetupError;

/// What a snapshot is, and why it is cheap. Shown first by `snapall setup` and in the TUI's help.
pub const WHAT_IS_A_SNAPSHOT: &str = "A snapshot is a saved moment of your computer. If an update, a new setting or a mistake breaks something, you can go back to how things were.\n\nA snapshot does not copy your files. The disk only remembers what changes afterwards, so a new snapshot takes almost no space, and it grows only as you change things. That is why it is quick, and why it can be kept for a long time.";

/// What an area is, and why it matters which ones are protected.
pub const WHAT_IS_AN_AREA: &str = "Snapshots are kept for one part of your disk at a time, called an area. A snapshot of one area does not contain the others: the System and your Home files are separate areas, so each needs protecting. snapall can only help with an area once it is protected.";

/// What a snapshot is not.
pub const NOT_A_BACKUP: &str = "Snapshots live on the same disk as your files. They protect you from mistakes and bad changes, not from the disk failing or being lost: for that you still need a copy somewhere else.";

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
fn when_it_runs(schedule: &[String]) -> String {
    if schedule.is_empty() {
        "on the schedule your system gives it".to_owned()
    } else {
        schedule.join(", ")
    }
}

/// What each area keeps and protects from cleanup, from its real settings; nothing if none is known.
fn limits_sentence(limits: &[Limit]) -> String {
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

/// One line for an area: its name, where it is, and whether it is protected.
pub fn area_line(area: &Area) -> String {
    let info = describe(&area.mount_point);
    let state = if area.is_protected() {
        "protected"
    } else {
        "NOT protected"
    };
    format!("{} ({}): {state}", info.title, area.mount_point)
}

/// What an area holds and whether it is worth protecting, for the list of areas.
pub fn area_detail(info: &Info) -> String {
    let verdict = match info.worth {
        Worth::Recommended => "Recommended.",
        Worth::Optional => "Up to you.",
        Worth::NotUseful => "Not worth protecting.",
    };
    format!(
        "It holds {}. {verdict} {}.",
        info.holds,
        capital(&info.reason)
    )
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// What protecting an area will do and will not do, for the person to read before they say yes.
pub fn protect_offer(mount_point: &str) -> String {
    let info = describe(mount_point);
    format!(
        "Protect {title} ({mount_point})?\n\n\
         It holds {holds}.\n\
         {verdict}: {reason}.\n\n\
         What snapall will do:\n  \
         - make a hidden folder called .snapshots inside {mount_point}, where its snapshots will be kept;\n  \
         - make sure snapshots of it are taken only when you ask (with snapall), never on a schedule, so nothing is taken or piles up on its own.\n\n\
         What it will not do: it does not copy, move or change any of your files, and it takes no snapshot yet.\n\n\
         Space: a snapshot takes almost nothing when it is taken and grows only as files change. How many are kept, and when the oldest are removed, are set for each area and shown in the Health tab.",
        title = info.title,
        holds = info.holds,
        reason = info.reason,
        verdict = match info.worth {
            Worth::Recommended => "Recommended",
            Worth::Optional => "Up to you",
            Worth::NotUseful => "Not recommended",
        },
    )
}

/// The offer for a window that is 24 lines tall: what it holds, whether it is worth it, what will and
/// will not happen. (How space and cleanup work is in the help and in `snapall setup`.)
pub fn protect_summary(mount_point: &str) -> String {
    let info = describe(mount_point);
    format!(
        "Protect {title} ({mount_point})?\n\n\
         It holds {holds}.\n\
         {verdict}: {reason}.\n\n\
         What snapall will do:\n  \
         - make a hidden folder called .snapshots inside {mount_point}, where its snapshots are kept;\n  \
         - make sure snapshots are taken only when you ask, never on a schedule.\n\n\
         It does not copy, move or change any of your files, and takes no snapshot yet.",
        title = info.title,
        holds = info.holds,
        reason = info.reason,
        verdict = match info.worth {
            Worth::Recommended => "Recommended",
            Worth::Optional => "Up to you",
            Worth::NotUseful => "Not recommended",
        },
    )
}

/// A failed attempt to protect an area, as a person should read it: what went wrong with what, and
/// whether anything was changed.
pub fn failure_text(title: &str, error: &SetupError) -> String {
    match error {
        SetupError::Failed(reason) => format!(
            "Could not protect {title}. The reason the system gave: {reason}. Nothing was changed."
        ),
        SetupError::LeftBehind { problem, area: config, .. } => format!(
            "Protecting {title} stopped half way ({problem}) and the part already done could not be taken back: it is left under the name `{config}`. Please report this at https://github.com/grenudi/snapall/issues."
        ),
        other => format!("Could not protect {title}: {other}. Nothing was changed."),
    }
}

/// What to say after an area was protected.
pub fn protected_text(mount_point: &str) -> String {
    let info = describe(mount_point);
    format!(
        "{} ({mount_point}) is protected now. No snapshot exists yet: the next time you take one (the snapall window, or `sudo snapall`), it will include this area.",
        info.title
    )
}

/// What to say about a deleted snapshot, which cannot be brought back: what protects against needing to.
pub const DELETE_ADVICE: &str = "A deleted snapshot cannot be brought back. What protects against needing to is a snapshot taken before a risky step: snapall create, or snapall create --group NAME to take one of a whole group.";

/// The same for an area that stopped being protected.
pub const UNPROTECT_ADVICE: &str = "Stopping protection cannot be undone: its snapshots are gone. snapall setup protects the area again, starting fresh with no snapshots. What protects against needing to is a snapshot taken before a risky step: snapall create, or snapall create --group NAME to take one of a whole group.";

/// What a restore puts back besides the files a person is thinking of, and what to do about it. Shown
/// wherever a restore is about to happen.
pub const RESTORE_CAUTION: &str = "Everything in the area goes back, including what programs keep there: their settings, saved sessions, databases and logs. Close the programs that use those files first, and if one file or folder is all you need, restore just that.";

/// Where a restore of the whole computer happens, in two sentences, for the help text.
pub const RESTORING: &str = "To go back, pick a snapshot and choose what to restore: one file, or a whole area such as Home. snapall always takes a snapshot of how things are first, so going back can itself be undone.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::Area;

    fn schedule() -> Vec<String> {
        vec![
            "10 minutes after the computer starts".into(),
            "then again 1 day after each run starts".into(),
        ]
    }

    fn limits() -> Vec<Limit> {
        vec![
            Limit {
                area: "Home".into(),
                keep: Some(50),
                min_age_seconds: Some(1800),
            },
            Limit {
                area: "System".into(),
                keep: Some(20),
                min_age_seconds: None,
            },
        ]
    }

    const JARGON: [&str; 7] = [
        "subvolume",
        "btrfs",
        "snapper",
        "config",
        "filesystem",
        "file system",
        "mountpoint",
    ];

    fn assert_plain(label: &str, text: &str) {
        let lower = text.to_lowercase();
        for word in JARGON {
            assert!(!lower.contains(word), "`{word}` in {label}: {text}");
        }
    }

    #[test]
    fn nothing_a_person_reads_assumes_they_know_the_vocabulary() {
        assert_plain("snapshot", WHAT_IS_A_SNAPSHOT);
        assert_plain("area", WHAT_IS_AN_AREA);
        assert_plain("backup", NOT_A_BACKUP);
        assert_plain("cleanup offer", &cleanup_offer(&schedule(), &limits()));
        assert_plain("restoring", RESTORING);
        for state in [Cleanup::On, Cleanup::Off, Cleanup::Unavailable] {
            assert_plain("cleanup", &cleanup_text(state, &schedule(), &limits()));
        }
        for path in ["/", "/home", "/root", "/srv", "/var/log", "/tmp", "/mnt/x"] {
            assert_plain("offer", &protect_offer(path).replace(".snapshots", ""));
            assert_plain("detail", &area_detail(&describe(path)));
            assert_plain("protected", &protected_text(path));
            assert_plain(
                "line",
                &area_line(&Area {
                    mount_point: path.into(),
                    protected_by: None,
                }),
            );
        }
    }

    #[test]
    fn the_short_offer_keeps_what_matters_and_is_short_enough_for_a_small_window() {
        let offer = protect_summary("/home");
        for must in [
            "Protect Home (/home)?",
            "your own files",
            "Recommended:",
            "never on a schedule",
            "does not copy, move or change any of your files",
            "takes no snapshot yet",
        ] {
            assert!(offer.contains(must), "`{must}` missing:\n{offer}");
        }
        assert!(
            offer.lines().count() <= 13,
            "{} lines:\n{offer}",
            offer.lines().count()
        );
        assert_plain("short offer", &offer.replace(".snapshots", ""));
    }

    #[test]
    fn a_failure_says_what_went_wrong_and_that_nothing_changed() {
        let error = SetupError::Failed(crate::port::BackendError::NotSupported(
            "path is not usable".into(),
        ));
        let text = failure_text("Home", &error);
        assert!(
            text.contains("Could not protect Home")
                && text.contains("path is not usable")
                && text.contains("Nothing was changed"),
            "{text}"
        );
    }

    #[test]
    fn the_offer_says_what_will_happen_and_what_will_not() {
        let offer = protect_offer("/home");
        for must in [
            "Protect Home (/home)?",
            "your own files",
            "could not be undone",
            "What snapall will do",
            "never on a schedule",
            "does not copy, move or change any of your files",
            "takes no snapshot yet",
            "grows only as files change",
        ] {
            assert!(offer.contains(must), "`{must}` missing:\n{offer}");
        }
    }

    #[test]
    fn an_area_that_is_not_worth_it_is_not_presented_as_a_reason_to_protect_it() {
        let offer = protect_offer("/var/log");
        assert!(
            offer.contains("Not recommended:") && !offer.contains("Recommended:\n"),
            "{offer}"
        );
        assert!(offer.contains("mostly waste space"), "{offer}");
    }

    #[test]
    fn automatic_cleanup_is_read_from_what_the_system_says_and_never_guessed() {
        assert_eq!(
            cleanup_from(Some("enabled\n"), Some("active\n")),
            Cleanup::On
        );
        assert_eq!(
            cleanup_from(Some("disabled"), Some("inactive")),
            Cleanup::Off
        );
        // enabled but not running is not "on": nothing is cleaning anything up
        assert_eq!(
            cleanup_from(Some("enabled"), Some("inactive")),
            Cleanup::Unavailable
        );
        assert_eq!(
            cleanup_from(Some("enabled"), Some("failed")),
            Cleanup::Unavailable
        );
        assert_eq!(cleanup_from(Some("enabled"), None), Cleanup::Unavailable);
        for other in [Some("masked"), Some("not-found"), Some(""), None] {
            assert_eq!(
                cleanup_from(other, Some("active")),
                Cleanup::Unavailable,
                "{other:?}"
            );
        }
        assert!(cleanup_text(Cleanup::Off, &[], &[]).contains("pile up"));
        assert!(
            cleanup_text(Cleanup::Unavailable, &[], &[]).contains("could not find out"),
            "an unknown state is not presented as a known one"
        );
    }

    #[test]
    fn a_line_says_plainly_whether_an_area_is_protected() {
        let home = Area {
            mount_point: "/home".into(),
            protected_by: None,
        };
        assert_eq!(area_line(&home), "Home (/home): NOT protected");
        let system = Area {
            mount_point: "/".into(),
            protected_by: Some(crate::domain::AreaName::parse("root").unwrap()),
        };
        assert_eq!(area_line(&system), "System (/): protected");
    }

    #[test]
    fn the_cleanup_text_is_made_from_the_real_schedule_and_limits_and_changes_when_they_do() {
        let text = cleanup_text(Cleanup::On, &schedule(), &limits());
        assert!(
            text.contains(
                "10 minutes after the computer starts, then again 1 day after each run starts"
            ),
            "{text}"
        );
        assert!(
            text.contains("Home keeps the newest 50 and never removes one younger than 30 minutes"),
            "{text}"
        );
        assert!(text.contains("System keeps the newest 20"), "{text}");
        // the same text with other facts says other things: nothing is typed in
        let other = vec![Limit {
            area: "Home".into(),
            keep: Some(7),
            min_age_seconds: Some(7200),
        }];
        let text = cleanup_text(Cleanup::On, &["every hour".into()], &other);
        assert!(
            text.contains("runs every hour")
                && text.contains("keeps the newest 7")
                && text.contains("2 hours"),
            "{text}"
        );
        assert!(
            !text.contains("50") && !text.contains("30 minutes") && !text.contains("once a day"),
            "{text}"
        );
    }

    #[test]
    fn when_nothing_is_known_nothing_is_made_up() {
        let text = cleanup_text(Cleanup::On, &[], &[]);
        assert!(
            text.contains("on the schedule your system gives it"),
            "{text}"
        );
        assert!(
            !text.contains("keeps the newest"),
            "no limit is claimed: {text}"
        );
        let offer = cleanup_offer(&[], &[]);
        assert!(
            offer.contains("on the schedule your system gives it") && !offer.contains("keeps"),
            "{offer}"
        );
        // a limit with no age says only what it knows
        let some = cleanup_text(
            Cleanup::Off,
            &[],
            &[Limit {
                area: "Home".into(),
                keep: Some(3),
                min_age_seconds: None,
            }],
        );
        assert!(
            some.contains("Home keeps the newest 3.") && !some.contains("younger"),
            "{some}"
        );
    }

    #[test]
    fn durations_are_said_in_the_largest_whole_unit() {
        for (seconds, words) in [
            (0, "0 seconds"),
            (1, "1 second"),
            (90, "90 seconds"),
            (1800, "30 minutes"),
            (3600, "1 hour"),
            (7200, "2 hours"),
            (86_400, "1 day"),
            (172_800, "2 days"),
        ] {
            assert_eq!(seconds_words(seconds), words);
        }
    }

    #[test]
    fn room_counting_is_explained_with_what_it_gives_what_it_costs_and_when_to_leave_it_off() {
        let off = room_counting_text(QuotaMode::Off);
        for must in [
            "not counting how much room each snapshot holds",
            "Switching counting on",
            "The price",
            "slow some operations down",
            "deleting snapshots most of all",
            "whole disk",
            "switched off again",
            "leaving it off is the better choice",
        ] {
            assert!(off.contains(must), "`{must}` missing: {off}");
        }
        let full = room_counting_text(QuotaMode::Full);
        assert!(
            full.contains("what each snapshot alone holds")
                && full.contains("The price")
                && full.contains("Switching it off"),
            "{full}"
        );
        let simple = room_counting_text(QuotaMode::Simple);
        assert!(
            simple.contains("cannot be told from it")
                && simple.contains("not something snapall can do in place"),
            "{simple}"
        );
        assert!(room_counting_text(QuotaMode::Unknown).contains("could not find out"));
        for mode in [
            QuotaMode::Off,
            QuotaMode::Full,
            QuotaMode::Simple,
            QuotaMode::Unknown,
        ] {
            assert_plain("room counting", &room_counting_text(mode));
        }
    }
}
