// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a job's unit file says about it. systemd merges the file with its drop-ins, later ones winning,
//! and an empty assignment (`OnCalendar=`) clears what came before; reading it the same way is what makes
//! the schedule shown the schedule that runs.

use std::path::{Path, PathBuf};

use snapall_core::UnitDetails;

/// Where unit files live, lowest precedence first.
pub const UNIT_DIRECTORIES: [&str; 4] = [
    "/lib/systemd/system",
    "/usr/lib/systemd/system",
    "/run/systemd/system",
    "/etc/systemd/system",
];

/// The unit file and its drop-ins, read from `directories` (lowest precedence first).
pub fn read(unit: &str, directories: &[PathBuf]) -> Option<UnitDetails> {
    let mut texts: Vec<String> = Vec::new();
    let mut file: Option<String> = None;
    for dir in directories {
        let path = dir.join(unit);
        if let Ok(text) = std::fs::read_to_string(&path) {
            texts.push(text);
            file = Some(path.display().to_string());
        }
    }
    file.as_ref()?;
    for dir in directories {
        texts.extend(drop_ins(&dir.join(format!("{unit}.d"))));
    }
    let mut details = parse(&texts.iter().map(String::as_str).collect::<Vec<_>>());
    details.file = file;
    Some(details)
}

/// The `*.conf` files of a drop-in directory, in name order.
fn drop_ins(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "conf"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect()
}

const TIMER_KEYS: [&str; 7] = [
    "OnActiveSec",
    "OnBootSec",
    "OnStartupSec",
    "OnUnitActiveSec",
    "OnUnitInactiveSec",
    "OnCalendar",
    "Persistent",
];

/// The description and the timer settings of these texts, applied in order.
pub fn parse(texts: &[&str]) -> UnitDetails {
    let mut details = UnitDetails::default();
    for text in texts {
        let mut section = "";
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = match name {
                    "Unit" => "Unit",
                    "Timer" => "Timer",
                    _ => "",
                };
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match section {
                "Unit" if key == "Description" => {
                    details.description = (!value.is_empty()).then(|| value.to_owned());
                }
                "Timer" if TIMER_KEYS.contains(&key) => {
                    // an empty assignment clears what the key had before
                    details.timer.retain(|(existing, _)| existing != key);
                    if !value.is_empty() {
                        details.timer.push((key.to_owned(), value.to_owned()));
                    }
                }
                _ => {}
            }
        }
    }
    details
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEANUP_TIMER: &str = "[Unit]\nDescription=Daily Cleanup of Snapper Snapshots\nDocumentation=man:snapper(8)\n[Timer]\nOnBootSec=10m\nOnUnitActiveSec=1d\n[Install]\nWantedBy=timers.target\n";

    #[test]
    fn a_unit_file_gives_its_description_and_its_schedule() {
        let details = parse(&[CLEANUP_TIMER]);
        assert_eq!(
            details.description.as_deref(),
            Some("Daily Cleanup of Snapper Snapshots")
        );
        assert_eq!(
            details.timer,
            [
                ("OnBootSec".into(), "10m".into()),
                ("OnUnitActiveSec".into(), "1d".into())
            ]
        );
        assert_eq!(
            details.schedule(),
            [
                "10 minutes after the computer starts",
                "then again 1 day after each run starts"
            ]
        );
    }

    #[test]
    fn a_drop_in_overrides_and_an_empty_assignment_clears_as_systemd_does() {
        let drop_in = "[Timer]\nOnUnitActiveSec=\nOnUnitActiveSec=12h\n";
        let details = parse(&[CLEANUP_TIMER, drop_in]);
        assert_eq!(
            details.schedule(),
            [
                "10 minutes after the computer starts",
                "then again 12 hours after each run starts"
            ]
        );
        let cleared = parse(&[
            CLEANUP_TIMER,
            "[Timer]\nOnBootSec=\nOnUnitActiveSec=\nOnCalendar=daily\n",
        ]);
        assert_eq!(cleared.schedule(), ["every day"]);
    }

    #[test]
    fn only_the_unit_and_timer_sections_count_and_comments_are_ignored() {
        let text = "# a comment\n[Service]\nOnBootSec=5m\nDescription=not this one\n[Unit]\n; another\nDescription=The real one\n";
        let details = parse(&[text]);
        assert_eq!(details.description.as_deref(), Some("The real one"));
        assert!(
            details.timer.is_empty(),
            "OnBootSec outside [Timer] is not a schedule"
        );
    }

    #[test]
    fn files_are_found_in_the_directories_given_and_the_highest_precedence_wins() {
        let root = std::env::temp_dir().join(format!("snapall-units-{}", std::process::id()));
        let (lib, etc) = (root.join("lib"), root.join("etc"));
        std::fs::create_dir_all(etc.join("job.timer.d")).unwrap();
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(
            lib.join("job.timer"),
            "[Unit]\nDescription=Shipped\n[Timer]\nOnCalendar=hourly\n",
        )
        .unwrap();
        std::fs::write(
            etc.join("job.timer.d/override.conf"),
            "[Timer]\nOnCalendar=\nOnCalendar=weekly\n",
        )
        .unwrap();
        let details = read("job.timer", &[lib.clone(), etc.clone()]).unwrap();
        assert_eq!(details.description.as_deref(), Some("Shipped"));
        assert_eq!(
            details.schedule(),
            ["every week"],
            "the override replaced the shipped schedule"
        );
        assert_eq!(
            details.file.as_deref(),
            Some(lib.join("job.timer").to_str().unwrap())
        );
        assert!(read("missing.timer", &[lib, etc]).is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
