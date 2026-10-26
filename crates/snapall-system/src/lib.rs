// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The system port, answered by asking the machine: `df` for a disk, `systemctl` for a background
//! job, `date` for the time. What each program prints is parsed by a small pure function, so it is
//! tested without the program; the asking itself is a few lines and is tested on the machine it runs on.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::path::Path;
use std::process::Command;

use snapall_core::{DiskUsage, ServiceState, SystemProbe};

/// The machine snapall is running on.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinuxSystem;

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

    fn enable_job(&self, unit: &str) -> Result<(), String> {
        let output = Command::new("systemctl")
            .args(["enable", "--now", unit])
            .output()
            .map_err(|e| format!("systemctl could not be run: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
        }
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
        let disk = LinuxSystem.disk_usage("/").expect("df works here");
        assert!(disk.size > 0 && disk.used <= disk.size, "{disk:?}");
        assert!(LinuxSystem.disk_usage("/definitely/not/a/path").is_none());
    }

    #[test]
    fn the_local_time_is_written_the_way_snapper_writes_dates() {
        let now = LinuxSystem.local_time().expect("date works here");
        assert!(seconds_from_snapper_date(&now).is_some(), "{now:?}");
    }

    #[test]
    fn a_job_that_does_not_exist_is_not_installed_whatever_the_machine_has() {
        let state = LinuxSystem.service("snapall-no-such-job.service");
        assert!(!state.is_installed() && !state.is_running(), "{state:?}");
    }

    #[test]
    fn files_are_found_or_not() {
        assert!(LinuxSystem.file_exists("/proc"));
        assert!(!LinuxSystem.file_exists("/no/such/file"));
    }
}
