// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagnosing the setup: what each check finds and what to do about it.

use super::*;
use crate::vocabulary::domain::AreaName;

/// Facts about the machine, gathered by the shell.
#[derive(Debug, Clone)]
pub struct Probe {
    pub os: OsRelease,
    /// `snapper --version`'s first line, or why it could not be run.
    pub snapper: Result<String, String>,
    /// Whether the effective user is root, if that could be determined.
    pub is_root: Option<bool>,
    /// The areas snapper reports, or why asking failed. `None` when snapper is missing.
    pub areas: Option<Result<Vec<AreaName>, String>>,
    /// The subvolume of each area (where it is mounted), for what the areas do not cover.
    pub locations: Vec<(AreaName, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    /// What to do about it, when there is something to do.
    pub fix: Option<String>,
}

pub(super) fn check(
    name: &'static str,
    status: Status,
    detail: impl Into<String>,
    fix: Option<String>,
) -> Check {
    Check {
        name,
        status,
        detail: detail.into(),
        fix,
    }
}

/// Turn the facts into findings, in the order a person would fix them.
pub fn diagnose(probe: &Probe) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(match &probe.snapper {
        Ok(version) => check("snapper", Status::Ok, version.clone(), None),
        Err(why) => check(
            "snapper",
            Status::Fail,
            format!("not usable: {why}"),
            Some(describe(&install_plan(&probe.os))),
        ),
    });

    checks.push(match probe.is_root {
        Some(true) => check("privileges", Status::Ok, "running as root", None),
        Some(false) => check(
            "privileges",
            Status::Warn,
            "not root; snapper normally needs root (or permission granted in its settings)",
            Some("run with sudo".to_owned()),
        ),
        None => check(
            "privileges",
            Status::Warn,
            "could not tell whether this is root",
            None,
        ),
    });

    match &probe.areas {
        None => checks.push(check(
            "areas",
            Status::Warn,
            "not checked: snapper is missing",
            None,
        )),
        Some(Ok(areas)) if areas.is_empty() => checks.push(check(
            "areas",
            Status::Warn,
            "No part of your disk is protected yet, so there is nothing to snapshot",
            Some(
                "run `sudo snapall setup`: it explains what protecting means and does it for you"
                    .to_owned(),
            ),
        )),
        Some(Ok(areas)) => {
            let names: Vec<&str> = areas.iter().map(AreaName::as_str).collect();
            checks.push(check(
                "areas",
                Status::Ok,
                format!("{} protected: {}", names.len(), names.join(", ")),
                None,
            ));
        }
        Some(Err(why)) => checks.push(check(
            "areas",
            Status::Fail,
            format!("could not list the protected areas: {why}"),
            area_fix(why, probe.is_root),
        )),
    }
    checks
}

/// What to try when asking snapper for its areas failed.
pub(super) fn area_fix(why: &str, is_root: Option<bool>) -> Option<String> {
    if why.to_ascii_lowercase().contains("dbus") {
        // snapper talks to snapperd over the system bus. Containers and minimal installs
        // have none; as root, libsnapper can be used directly.
        return Some(
            "the system D-Bus (snapperd) is not reachable; as root, retry with --no-dbus"
                .to_owned(),
        );
    }
    (is_root == Some(false)).then(|| "run with sudo".to_owned())
}

/// The worst status among the checks (`Ok` when there are none).
pub fn worst(checks: &[Check]) -> Status {
    checks.iter().map(|c| c.status).max().unwrap_or(Status::Ok)
}
