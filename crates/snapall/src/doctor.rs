// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The imperative shell for `snapall doctor`: probe the machine, run the install steps.
//!
//! All decisions (what is wrong, what to install, with which command) are pure functions in
//! `snapall_core::doctor`; this module only gathers facts and performs the I/O.

use std::error::Error;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::{Command, ExitCode};

use snapall_core::doctor::{diagnose, install_plan, worst, InstallPlan, OsRelease, Probe, Status};
use snapall_core::SnapshotBackend;

use crate::{emit, emit_json, render};

/// Gather the facts about this machine.
pub fn probe(backend: &dyn SnapshotBackend) -> Probe {
    let snapper = snapper_version();
    let configs = snapper
        .is_ok()
        .then(|| backend.list_configs().map_err(|e| e.to_string()));
    Probe {
        os: read_os_release(),
        snapper,
        is_root: std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| effective_uid(&status))
            .map(|uid| uid == 0),
        configs,
    }
}

fn read_os_release() -> OsRelease {
    ["/etc/os-release", "/usr/lib/os-release"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| OsRelease::parse(&text))
        .unwrap_or_default()
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

fn snapper_version() -> Result<String, String> {
    match Command::new("snapper").arg("--version").output() {
        Ok(out) if out.status.success() => Ok(first_line(&out.stdout)),
        Ok(out) => {
            let why = first_line(&out.stderr);
            Err(if why.is_empty() {
                out.status.to_string()
            } else {
                why
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

/// The effective uid from the text of `/proc/self/status` (`Uid: real effective saved fs`).
fn effective_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().nth(1))
        .and_then(|uid| uid.parse().ok())
}

/// Ask on the terminal. Refuses to guess when there is nobody to ask.
fn confirm() -> Result<bool, Box<dyn Error>> {
    if !io::stdin().is_terminal() {
        return Err("not a terminal, so I cannot ask; pass --yes to run without asking".into());
    }
    eprint!("Proceed? [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// Run the install plan. `Ok(false)` means "did not install, and said why".
fn install(probe: &Probe, yes: bool) -> Result<bool, Box<dyn Error>> {
    match install_plan(&probe.os) {
        InstallPlan::Manual(text) => {
            eprintln!("snapall: {text}");
            Ok(false)
        }
        InstallPlan::Steps(steps) => {
            if probe.is_root != Some(true) {
                return Err(
                    "installing packages needs root; run `sudo snapall doctor --install`".into(),
                );
            }
            eprintln!("snapall will run, as root:");
            for step in &steps {
                eprintln!("  {}", step.display());
            }
            if !yes && !confirm()? {
                eprintln!("snapall: not installing");
                return Ok(false);
            }
            for step in &steps {
                let status = Command::new(step.program)
                    .args(&step.args)
                    .status()
                    .map_err(|e| format!("could not run `{}`: {e}", step.program))?;
                if !status.success() {
                    return Err(format!("`{}` failed ({status})", step.display()).into());
                }
            }
            Ok(true)
        }
    }
}

/// `snapall doctor [--install [--yes]] [--json]`.
pub fn run(
    backend: &dyn SnapshotBackend,
    install_missing: bool,
    yes: bool,
    json: bool,
) -> Result<ExitCode, Box<dyn Error>> {
    let mut probe = probe(backend);
    if install_missing {
        if probe.snapper.is_ok() {
            eprintln!("snapall: snapper is already installed; nothing to install");
        } else if install(&probe, yes)? {
            // Judge the result by looking again, not by trusting the exit status.
            probe = self::probe(backend);
        } else {
            // Say what is wrong too, so the manual guidance comes with the full picture.
            emit_report(&probe, json)?;
            return Ok(ExitCode::FAILURE);
        }
    }
    emit_report(&probe, json)?;
    let checks = diagnose(&probe);
    Ok(if worst(&checks) == Status::Fail {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn emit_report(probe: &Probe, json: bool) -> Result<(), Box<dyn Error>> {
    let checks = diagnose(probe);
    if json {
        emit_json(&render::doctor_json(&probe.os, &checks))
    } else {
        emit(&render::doctor(&probe.os, &checks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_effective_uid_is_the_second_field() {
        let status =
            "Name:\tsnapall\nUmask:\t0022\nUid:\t1000\t0\t0\t0\nGid:\t1000\t1000\t1000\t1000\n";
        assert_eq!(effective_uid(status), Some(0));
        assert_eq!(effective_uid("Uid:\t0\t1000\t0\t0\n"), Some(1000));
    }

    #[test]
    fn an_unreadable_status_is_unknown_not_root() {
        assert_eq!(effective_uid(""), None);
        assert_eq!(effective_uid("Uid:\n"), None);
        assert_eq!(effective_uid("Uid:\tx\ty\n"), None);
    }

    #[test]
    fn only_the_first_line_of_a_command_output_is_used() {
        assert_eq!(
            first_line(b"snapper 0.13.1\nlibsnapper 7.2.0\n"),
            "snapper 0.13.1"
        );
        assert_eq!(first_line(b""), "");
    }
}
