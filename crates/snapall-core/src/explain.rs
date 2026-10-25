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

/// What a snapshot is, and why it is cheap. Shown first by `snapall setup` and in the TUI's help.
pub const WHAT_IS_A_SNAPSHOT: &str = "A snapshot is a saved moment of your computer. If an update, a new setting or a mistake breaks something, you can go back to how things were.\n\nA snapshot does not copy your files. The disk only remembers what changes afterwards, so a new snapshot takes almost no space, and it grows only as you change things. That is why it is quick, and why it can be kept for a long time.";

/// What an area is, and why it matters which ones are protected.
pub const WHAT_IS_AN_AREA: &str = "Snapshots are kept for one part of your disk at a time, called an area. A snapshot of one area does not contain the others: the System and your Home files are separate areas, so each needs protecting. snapall can only help with an area once it is protected.";

/// What a snapshot is not.
pub const NOT_A_BACKUP: &str = "Snapshots live on the same disk as your files. They protect you from mistakes and bad changes, not from the disk failing or being lost: for that you still need a copy somewhere else.";

/// How automatic cleanup stands on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cleanup {
    /// The daily job that removes the oldest snapshots runs.
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

/// What automatic cleanup means for the person, for this computer.
pub fn cleanup_text(state: Cleanup) -> &'static str {
    match state {
        Cleanup::On => "Old snapshots are removed automatically: a background job runs once a day and, when an area has more than 50, removes the oldest. Nothing piles up.",
        Cleanup::Off => "Old snapshots are NOT removed automatically on this computer. The background job that does it is switched off, so snapshots would pile up and use more and more space.",
        Cleanup::Unavailable => "snapall could not find out whether old snapshots are removed automatically on this computer. If they are not, they pile up: you can delete them yourself in the snapall window.",
    }
}

/// What switching cleanup on does, to ask before doing it.
pub const CLEANUP_OFFER: &str = "Switch it on? This starts the small background job that runs once a day and removes the oldest snapshots when an area has more than 50 (never one younger than half an hour). It touches nothing else.";

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
         Space: a snapshot takes almost nothing when it is taken and grows only as files change. By default the newest 50 are kept for each area and older ones are removed.",
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

/// What to say after an area was protected.
pub fn protected_text(mount_point: &str) -> String {
    let info = describe(mount_point);
    format!(
        "{} ({mount_point}) is protected now. No snapshot exists yet: the next time you take one (the snapall window, or `sudo snapall`), it will include this area.",
        info.title
    )
}

/// Where a restore of the whole computer happens, in two sentences, for the help text.
pub const RESTORING: &str = "To go back, pick a snapshot and choose what to restore: one file, or a whole area such as Home. snapall always takes a snapshot of how things are first, so going back can itself be undone.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::Area;

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
        assert_plain("cleanup offer", CLEANUP_OFFER);
        assert_plain("restoring", RESTORING);
        for state in [Cleanup::On, Cleanup::Off, Cleanup::Unavailable] {
            assert_plain("cleanup", cleanup_text(state));
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
        assert!(cleanup_text(Cleanup::Off).contains("pile up"));
        assert!(
            cleanup_text(Cleanup::Unavailable).contains("could not find out"),
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
            protected_by: Some(crate::domain::ConfigName::parse("root").unwrap()),
        };
        assert_eq!(area_line(&system), "System (/): protected");
    }
}
