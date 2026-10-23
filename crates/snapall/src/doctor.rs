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
use std::time::Instant;

use snapall_core::doctor::{
    diagnose, install_plan, worst, InstallPlan, OsRelease, Probe, Status, Step,
};
use snapall_core::{Event, Observer, SnapshotBackend};

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

/// Install steps the user agreed to. The only way to obtain one is [`approve`], so running steps
/// nobody approved is not a state this program can be in (the canon's typestate).
struct Approved(Vec<Step>);

/// Show the steps and get consent: `--yes`, or an answer on the terminal. `Ok(None)` is a "no".
fn approve(steps: Vec<Step>, yes: bool) -> Result<Option<Approved>, Box<dyn Error>> {
    eprintln!("snapall will run, as root:");
    for step in &steps {
        eprintln!("  {}", step.display());
    }
    if !yes && !confirm()? {
        eprintln!("snapall: not installing");
        return Ok(None);
    }
    Ok(Some(Approved(steps)))
}

/// Run approved steps in order, reporting each one (they change the system as root, so each is
/// worth an audit line), and stop at the first failure.
fn execute(approved: Approved, observer: &dyn Observer) -> Result<(), Box<dyn Error>> {
    for step in &approved.0 {
        let started = Instant::now();
        let result = match Command::new(step.program).args(&step.args).status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("exited with {status}")),
            Err(e) => Err(format!("could not run `{}`: {e}", step.program)),
        };
        observer.observe(&Event::InstallStepRun {
            command: step.display(),
            result: result.clone(),
            took: started.elapsed(),
        });
        result.map_err(|why| format!("`{}` failed ({why})", step.display()))?;
    }
    Ok(())
}

/// Install what is missing. `Ok(false)` means "did not install, and said why".
fn install(probe: &Probe, yes: bool, observer: &dyn Observer) -> Result<bool, Box<dyn Error>> {
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
            match approve(steps, yes)? {
                Some(approved) => {
                    execute(approved, observer)?;
                    Ok(true)
                }
                None => Ok(false),
            }
        }
    }
}

/// `snapall doctor [--install [--yes]] [--json]`.
pub fn run(
    backend: &dyn SnapshotBackend,
    observer: &dyn Observer,
    install_missing: bool,
    yes: bool,
    json: bool,
) -> Result<ExitCode, Box<dyn Error>> {
    let mut probe = probe(backend);
    if install_missing {
        if probe.snapper.is_ok() {
            eprintln!("snapall: snapper is already installed; nothing to install");
        } else if install(&probe, yes, observer)? {
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
