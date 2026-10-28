// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `snapall doctor` at work: looking at the machine, and running the steps that install what is missing.
//!
//! All decisions (what is wrong, what to install, with which command) are pure functions in
//! `snapall_core::protecting::doctor`; this module only gathers facts and performs the I/O.

use std::error::Error;
use std::process::ExitCode;
use std::time::Instant;

use snapall_core::going_back::boot::{self, BootStatus};
use snapall_core::protecting::coverage;
use snapall_core::protecting::doctor::{
    diagnose, install_plan, worst, Check, InstallPlan, Probe, Status, Step,
};
use snapall_core::{Event, Observer, SnapshotBackend};

use crate::output::{emit, emit_json};
use crate::render;
use snapall_system::{boot_menu, environment, installing};

/// Gather the facts about this machine.
pub fn probe(backend: &dyn SnapshotBackend) -> Probe {
    let snapper = snapall_snapper::installed_version();
    let configs = snapper
        .is_ok()
        .then(|| backend.list_areas().map_err(|e| e.to_string()));
    // Where each area lives, to see what none of them covers. Unreadable ones are left out: this
    // only adds a finding, and the areas check already says if listing fails.
    let locations = match &configs {
        Some(Ok(names)) => names
            .iter()
            .filter_map(|name| Some((name.clone(), backend.area_location(name).ok()?)))
            .collect(),
        _ => Vec::new(),
    };
    Probe {
        os: environment::operating_system(),
        snapper,
        is_root: environment::is_administrator(),
        areas: configs,
        locations,
    }
}

/// Ask on the terminal. Refuses to guess when there is nobody to ask.
fn confirm() -> Result<bool, Box<dyn Error>> {
    if !crate::ask::can_ask() {
        return Err("not a terminal, so I cannot ask; pass --yes to run without asking".into());
    }
    crate::ask::yes_no("Proceed?")
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
        let result = installing::run_step(step);
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

/// Can this system boot its snapshots? A row of the report, never a failure: it is information
/// about what to add, not a sign that snapper is broken.
fn boot_menu_check() -> Option<Check> {
    let status = boot::assess(&boot_menu::on_this_machine());
    let mut lines = boot::details(&status).into_iter();
    let detail = lines.next()?;
    let rest: Vec<String> = lines.collect();
    Some(Check {
        name: "boot-menu",
        status: if matches!(status, BootStatus::Active { .. }) {
            Status::Ok
        } else {
            Status::Warn
        },
        detail,
        fix: (!rest.is_empty()).then(|| rest.join("\n      ")),
    })
}

/// Is any part of the disk where a mistake could not be undone left without snapshots? Nothing is
/// broken, so this is a warning and never a failure; it says what is left out in plain words and
/// points at the one command that sets it up and explains it.
fn protection_check(probe: &Probe) -> Option<Check> {
    if !matches!(probe.areas, Some(Ok(_))) {
        return None;
    }
    let mountinfo = environment::mounts()?;
    let areas = coverage::areas(
        &coverage::mounts_from_mountinfo(&mountinfo),
        &probe.locations,
    );
    let gaps = coverage::gaps(&areas);
    if gaps.is_empty() {
        return None;
    }
    let detail = gaps
        .iter()
        .map(|area| {
            let info = coverage::describe(&area.mount_point);
            format!(
                "{} ({}) is not protected: {}",
                info.title, area.mount_point, info.reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n      ");
    Some(Check {
        name: "protection",
        status: Status::Warn,
        detail,
        fix: Some(
            "run `sudo snapall setup`: it explains what protecting means and does it for you"
                .into(),
        ),
    })
}

fn emit_report(probe: &Probe, json: bool) -> Result<(), Box<dyn Error>> {
    let mut checks = diagnose(probe);
    checks.extend(protection_check(probe));
    checks.extend(boot_menu_check());
    if json {
        emit_json(&render::doctor_json(&probe.os, &checks))
    } else {
        emit(&render::doctor(&probe.os, &checks))
    }
}
