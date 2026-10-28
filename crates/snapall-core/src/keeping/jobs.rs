// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The background jobs snapall depends on or watches: one catalogue, so that a name is written once.
//! Everything that mentions a job (how it is asked after, the commands that check on it and control
//! it, the words for its schedule) is derived from the entry here and from what the machine says about
//! it, never typed again somewhere else.

/// A background job of the system, as snapall knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Job {
    /// The systemd unit that is the job: what `systemctl` is asked about.
    pub unit: &'static str,
    /// What a person calls it.
    pub title: &'static str,
    /// What it does, in a sentence that assumes nothing.
    pub purpose: &'static str,
}

/// Removes the oldest snapshots of each area once it has more than its limit.
pub const CLEANUP: Job = Job {
    unit: "snapper-cleanup.timer",
    title: "Cleaning up old snapshots",
    purpose: "removes the oldest snapshots of each area once it has more than its limit",
};

/// Takes snapshots by itself, on a schedule, of the areas that have that switched on.
pub const SCHEDULED_SNAPSHOTS: Job = Job {
    unit: "snapper-timeline.timer",
    title: "Snapshots on a schedule",
    purpose: "takes snapshots by itself, on a schedule, of the areas that have it switched on",
};

/// Takes a snapshot of the system each time the computer starts.
pub const SNAPSHOT_AT_START: Job = Job {
    unit: "snapper-boot.timer",
    title: "A snapshot at every start",
    purpose: "takes a snapshot of the system each time the computer starts",
};

/// Adds each new snapshot of the system to the boot menu.
pub const BOOT_MENU: Job = Job {
    unit: "limine-snapper-sync.service",
    title: "The boot menu following snapshots",
    purpose: "adds each new snapshot of the system to the boot menu, so the computer can be started from an earlier one",
};

/// Answers snapshot requests from programs; started when needed.
pub const SNAPSHOT_SERVICE: Job = Job {
    unit: "snapperd.service",
    title: "The snapshot service",
    purpose: "answers requests about snapshots from programs such as this one, and is started when one asks",
};

/// Every job snapall watches, in the order they are shown.
pub const ALL: [Job; 5] = [
    CLEANUP,
    SCHEDULED_SNAPSHOTS,
    SNAPSHOT_AT_START,
    BOOT_MENU,
    SNAPSHOT_SERVICE,
];

/// What can be done to a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobAction {
    /// Switch it on for every start of the computer, and start it now.
    EnableNow,
    /// Switch it off for every start, and stop it now.
    DisableNow,
    /// Start it now; it stops again at the next restart unless it is enabled.
    Start,
    Stop,
    Restart,
}

impl JobAction {
    pub const ALL: [JobAction; 5] = [
        JobAction::EnableNow,
        JobAction::DisableNow,
        JobAction::Start,
        JobAction::Stop,
        JobAction::Restart,
    ];

    /// The `systemctl` words that do it. The commands shown to a person and the ones run are both made
    /// from these, so they cannot differ.
    pub fn systemctl(self) -> &'static [&'static str] {
        match self {
            JobAction::EnableNow => &["enable", "--now"],
            JobAction::DisableNow => &["disable", "--now"],
            JobAction::Start => &["start"],
            JobAction::Stop => &["stop"],
            JobAction::Restart => &["restart"],
        }
    }

    /// A few words for a list.
    pub fn label(self) -> &'static str {
        match self {
            JobAction::EnableNow => "Switch on and start now",
            JobAction::DisableNow => "Switch off and stop now",
            JobAction::Start => "Start now",
            JobAction::Stop => "Stop now",
            JobAction::Restart => "Restart",
        }
    }

    /// What it does, in words.
    pub fn means(self) -> &'static str {
        match self {
            JobAction::EnableNow => {
                "Switch it on and start it now; it then starts with the computer"
            }
            JobAction::DisableNow => {
                "Switch it off and stop it now; it no longer starts with the computer"
            }
            JobAction::Start => "Start it now, until the computer is restarted",
            JobAction::Stop => "Stop it now, until the computer is restarted",
            JobAction::Restart => "Stop it and start it again",
        }
    }
}

/// A command a person can type, and what it is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub what: &'static str,
    pub line: String,
}

impl Job {
    pub fn is_timer(&self) -> bool {
        self.unit.ends_with(".timer")
    }

    /// The unit that does the work: a timer only decides *when*; its service is what runs, and what
    /// has the log.
    pub fn worker_unit(&self) -> String {
        match self.unit.strip_suffix(".timer") {
            Some(name) => format!("{name}.service"),
            None => self.unit.to_owned(),
        }
    }

    /// The command a person types to do `action` to this job.
    pub fn command_for(&self, action: JobAction) -> String {
        format!(
            "sudo systemctl {} {}",
            action.systemctl().join(" "),
            self.unit
        )
    }

    /// The command that runs a timer's work right now, without waiting for its time.
    pub fn run_now_command(&self) -> Option<String> {
        self.is_timer()
            .then(|| format!("sudo systemctl start {}", self.worker_unit()))
    }

    /// The commands that check on this job and control it, made from its unit name.
    pub fn commands(&self) -> Vec<Command> {
        let unit = self.unit;
        let mut commands = vec![
            Command {
                what: "See how it stands",
                line: format!("systemctl status {unit}"),
            },
            Command {
                what: "Read what it reported",
                line: format!("journalctl -u {} -n 50", self.worker_unit()),
            },
        ];
        if let Some(line) = self.run_now_command() {
            commands.push(Command {
                what: "Run it right now, without waiting for its time",
                line,
            });
        }
        for action in JobAction::ALL {
            commands.push(Command {
                what: action.means(),
                line: self.command_for(action),
            });
        }
        commands
    }
}

// ---- schedules, in words --------------------------------------------------------------------------

/// `10m` as `10 minutes`, `1d` as `1 day`, `1h30min` as `1 hour 30 minutes`: systemd's time spans in
/// words. `None` for what is not a time span.
pub fn span_words(span: &str) -> Option<String> {
    let mut words = Vec::new();
    let mut rest = span.trim();
    if rest.is_empty() {
        return None;
    }
    while !rest.is_empty() {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let (number, after) = rest.split_at(digits);
        let letters = after
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .count();
        let (unit, next) = after.split_at(letters);
        let name = match unit {
            "" | "s" | "sec" | "second" | "seconds" => "second",
            "ms" | "msec" => "millisecond",
            "us" | "usec" => "microsecond",
            "m" | "min" | "minute" | "minutes" => "minute",
            "h" | "hr" | "hour" | "hours" => "hour",
            "d" | "day" | "days" => "day",
            "w" | "week" | "weeks" => "week",
            "M" | "month" | "months" => "month",
            "y" | "year" | "years" => "year",
            _ => return None,
        };
        let plural = if number == "1" { "" } else { "s" };
        words.push(format!("{number} {name}{plural}"));
        rest = next.trim_start();
    }
    Some(words.join(" "))
}

/// One timer setting of a unit file (`OnBootSec=10m`) as a phrase.
pub fn timer_setting_words(key: &str, value: &str) -> Option<String> {
    let span = || span_words(value);
    Some(match key {
        "OnBootSec" => format!("{} after the computer starts", span()?),
        "OnStartupSec" => format!("{} after the system manager starts", span()?),
        "OnActiveSec" => format!("{} after the timer is switched on", span()?),
        "OnUnitActiveSec" => format!("then again {} after each run starts", span()?),
        "OnUnitInactiveSec" => format!("then again {} after each run ends", span()?),
        "OnCalendar" => match value {
            "minutely" => "every minute".to_owned(),
            "hourly" => "every hour".to_owned(),
            "daily" => "every day".to_owned(),
            "weekly" => "every week".to_owned(),
            "monthly" => "every month".to_owned(),
            other => format!("on the calendar schedule `{other}`"),
        },
        _ => return None,
    })
}

/// The whole schedule of a timer, from its settings, in words.
pub fn schedule_words(settings: &[(String, String)]) -> Vec<String> {
    settings
        .iter()
        .filter_map(|(key, value)| timer_setting_words(key, value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn systemd_time_spans_are_said_in_words_and_what_is_not_a_span_is_refused() {
        for (span, words) in [
            ("10m", "10 minutes"),
            ("1d", "1 day"),
            ("1h30min", "1 hour 30 minutes"),
            ("90", "90 seconds"),
            ("2weeks", "2 weeks"),
            ("1", "1 second"),
        ] {
            assert_eq!(span_words(span).as_deref(), Some(words), "{span}");
        }
        for bad in ["", "soon", "m10", "5 parsecs", "10x"] {
            assert_eq!(span_words(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_timers_settings_become_the_schedule_a_person_reads() {
        let settings = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect()
        };
        // snapper's cleanup timer, as its unit file has it
        assert_eq!(
            schedule_words(&settings(&[
                ("OnBootSec", "10m"),
                ("OnUnitActiveSec", "1d")
            ])),
            [
                "10 minutes after the computer starts",
                "then again 1 day after each run starts"
            ]
        );
        assert_eq!(
            schedule_words(&settings(&[("OnCalendar", "hourly")])),
            ["every hour"]
        );
        assert_eq!(
            schedule_words(&settings(&[("OnCalendar", "*-*-* 03:00")])),
            ["on the calendar schedule `*-*-* 03:00`"]
        );
        assert!(
            schedule_words(&settings(&[("Persistent", "true")])).is_empty(),
            "not a schedule"
        );
    }

    #[test]
    fn every_command_is_made_from_the_unit_name_and_nothing_is_typed_twice() {
        for job in ALL {
            for command in job.commands() {
                assert!(
                    command.line.contains(job.unit) || command.line.contains(&job.worker_unit()),
                    "{}",
                    command.line
                );
            }
            // each action's command is exactly what the shell would run for it
            for action in JobAction::ALL {
                let wanted = format!(
                    "sudo systemctl {} {}",
                    action.systemctl().join(" "),
                    job.unit
                );
                assert!(job.commands().iter().any(|c| c.line == wanted), "{wanted}");
            }
        }
        let cleanup = CLEANUP.commands();
        assert_eq!(cleanup[0].line, "systemctl status snapper-cleanup.timer");
        assert_eq!(
            cleanup[1].line, "journalctl -u snapper-cleanup.service -n 50",
            "the log is the worker's"
        );
        assert!(
            cleanup
                .iter()
                .any(|c| c.line == "sudo systemctl start snapper-cleanup.service"),
            "run it now"
        );
        // a service is not a timer: no "run it now", and its log is its own
        let sync = BOOT_MENU.commands();
        assert_eq!(
            sync[1].line,
            "journalctl -u limine-snapper-sync.service -n 50"
        );
        assert!(!sync.iter().any(|c| c.what.starts_with("Run it right now")));
    }

    #[test]
    fn the_catalogue_names_each_unit_once() {
        let mut units: Vec<&str> = ALL.iter().map(|job| job.unit).collect();
        units.sort_unstable();
        units.dedup();
        assert_eq!(units.len(), ALL.len());
        assert!(ALL
            .iter()
            .all(|job| !job.title.is_empty() && !job.purpose.is_empty()));
    }
}
