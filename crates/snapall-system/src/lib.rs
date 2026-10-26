// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The system port, answered by asking the machine: `df` for a disk, `systemctl` for a background
//! job, `date` for the time, unit files for a job's schedule, `btrfs` for how room is counted, and a lock
//! file for who is making a change. What each program prints is parsed by a small pure function, so it is
//! tested without the program; the asking itself is a few lines and is tested on the machine it runs on.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod lock;
mod quotas;
mod units;

use std::path::{Path, PathBuf};
use std::process::Command;

use snapall_core::{
    Busy, DiskUsage, Hold, JobAction, QuotaMode, ServiceState, SystemProbe, UnitDetails,
};

/// The machine snapall is running on.
#[derive(Debug, Clone, Default)]
pub struct LinuxSystem {
    lock_path: Option<PathBuf>,
}

/// Where the lock is kept: `/run` is emptied at every start, so a crash cannot leave it for ever.
const LOCK_PATH: &str = "/run/snapall/change.lock";

impl LinuxSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep the change lock somewhere else (for tests).
    pub fn with_lock_path(path: PathBuf) -> Self {
        Self {
            lock_path: Some(path),
        }
    }

    fn lock_path(&self) -> PathBuf {
        self.lock_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(LOCK_PATH))
    }
}

impl SystemProbe for LinuxSystem {
    fn disk_usage(&self, path: &str) -> Option<DiskUsage> {
        parse_df(&ask_succeeding(
            "df",
            &["-B1", "--output=size,used", "--", path],
        )?)
    }

    fn service(&self, unit: &str) -> ServiceState {
        ServiceState {
            enabled: systemctl_says("is-enabled", unit),
            active: systemctl_says("is-active", unit),
        }
    }

    fn local_time(&self) -> Option<String> {
        let text = ask_succeeding("date", &["+%Y-%m-%d %H:%M:%S"])?;
        Some(text.trim().to_owned())
    }

    fn file_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn unit_details(&self, unit: &str) -> Option<UnitDetails> {
        let directories: Vec<PathBuf> = units::UNIT_DIRECTORIES.iter().map(PathBuf::from).collect();
        units::read(unit, &directories)
    }

    fn control_job(&self, unit: &str, action: JobAction) -> Result<(), String> {
        let output = Command::new("systemctl")
            .args(action.systemctl())
            .arg(unit)
            .output()
            .map_err(|e| format!("systemctl could not be run: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(plain_systemctl(&String::from_utf8_lossy(&output.stderr)))
        }
    }

    fn quota_mode(&self, path: &str) -> QuotaMode {
        quotas::mode(path)
    }

    fn set_quotas(&self, path: &str, on: bool) -> Result<(), String> {
        quotas::set(path, on)
    }

    fn hold_changes(&self, doing: &str) -> Result<Hold, Busy> {
        let since = self.local_time().unwrap_or_default();
        lock::hold(&self.lock_path(), doing, &since)
    }
}

/// What a program printed, if it ran and succeeded.
fn ask_succeeding(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What `systemctl is-enabled` or `is-active` printed. They print the state on a failing exit too
/// (`disabled`, `inactive`, `not-found`), so the exit status is not what decides: nothing printed means
/// systemctl is not there to ask.
fn systemctl_says(question: &str, unit: &str) -> Option<String> {
    let output = Command::new("systemctl")
        .args([question, unit])
        .output()
        .ok()?;
    let said = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!said.is_empty()).then_some(said)
}

/// systemctl's complaint, with the common one said in plain words.
fn plain_systemctl(stderr: &str) -> String {
    let text = stderr.trim();
    if text.contains("Access denied") || text.contains("Interactive authentication required") {
        "this needs administrator rights: start snapall with sudo".to_owned()
    } else if text.contains("not been booted with systemd") {
        "this computer was not started with systemd, so it cannot control background jobs"
            .to_owned()
    } else {
        text.to_owned()
    }
}

/// `df -B1 --output=size,used`: a heading line, then the two numbers.
pub fn parse_df(text: &str) -> Option<DiskUsage> {
    let mut numbers = text.lines().last()?.split_whitespace();
    let size = numbers.next()?.parse().ok()?;
    let used = numbers.next()?.parse().ok()?;
    numbers.next().is_none().then_some(DiskUsage { size, used })
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::calendar::seconds_from_snapper_date;

    #[test]
    fn what_df_prints_is_read_and_what_it_does_not_is_refused() {
        let real = "1B-blocks     Used\n536870912 10264576\n";
        assert_eq!(
            parse_df(real),
            Some(DiskUsage {
                size: 536_870_912,
                used: 10_264_576
            })
        );
        for bad in [
            "",
            "1B-blocks Used\n",
            "1B-blocks Used\nx y\n",
            "1B-blocks Used\n1 2 3\n",
            "1B-blocks Used\n5\n",
        ] {
            assert_eq!(parse_df(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_disk_holding_a_real_path_is_read() {
        let disk = LinuxSystem::new().disk_usage("/").expect("df works here");
        assert!(disk.size > 0 && disk.used <= disk.size, "{disk:?}");
        assert!(LinuxSystem::new()
            .disk_usage("/definitely/not/a/path")
            .is_none());
    }

    #[test]
    fn the_local_time_is_written_the_way_snapper_writes_dates() {
        let now = LinuxSystem::new().local_time().expect("date works here");
        assert!(seconds_from_snapper_date(&now).is_some(), "{now:?}");
    }

    #[test]
    fn a_job_that_does_not_exist_is_not_installed_whatever_the_machine_has() {
        let state = LinuxSystem::new().service("snapall-no-such-job.service");
        assert!(!state.is_installed() && !state.is_running(), "{state:?}");
        assert!(LinuxSystem::new()
            .unit_details("snapall-no-such-job.service")
            .is_none());
    }

    #[test]
    fn the_real_cleanup_timer_describes_itself_if_snapper_is_installed() {
        // On a machine with snapper, its unit file is the authority on the schedule.
        if let Some(details) = LinuxSystem::new().unit_details(snapall_core::services::CLEANUP.unit)
        {
            assert!(details.description.is_some(), "{details:?}");
            assert!(!details.schedule().is_empty(), "{details:?}");
        }
    }

    #[test]
    fn files_are_found_or_not() {
        assert!(LinuxSystem::new().file_exists("/proc"));
        assert!(!LinuxSystem::new().file_exists("/no/such/file"));
    }

    #[test]
    fn systemctls_common_complaints_are_said_plainly() {
        assert!(plain_systemctl("Failed to enable unit: Access denied")
            .contains("administrator rights"));
        assert!(
            plain_systemctl("System has not been booted with systemd as init system")
                .contains("not started with systemd")
        );
        assert_eq!(plain_systemctl(" other \n"), "other");
    }

    #[test]
    fn the_change_lock_is_taken_through_the_port_and_let_go() {
        let path = std::env::temp_dir()
            .join(format!("snapall-port-lock-{}", std::process::id()))
            .join("c.lock");
        let system = LinuxSystem::with_lock_path(path.clone());
        let held = system.hold_changes("taking a snapshot").unwrap();
        assert!(path.exists());
        drop(held);
        assert!(!path.exists());
    }
}
