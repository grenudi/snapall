// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running a program and reading what it says: what it printed, what the service manager says of a unit, and
//! the numbers `df` prints.

use std::process::Command;

use snapall_core::DiskUsage;

/// What a program printed, if it ran and succeeded.
pub(crate) fn ask_succeeding(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What `systemctl is-enabled` or `is-active` printed. They print the state on a failing exit too
/// (`disabled`, `inactive`, `not-found`), so the exit status is not what decides: nothing printed means
/// systemctl is not there to ask.
pub(crate) fn systemctl_says(question: &str, unit: &str) -> Option<String> {
    let output = Command::new("systemctl")
        .args([question, unit])
        .output()
        .ok()?;
    let said = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!said.is_empty()).then_some(said)
}

/// systemctl's complaint, with the common one said in plain words.
pub(crate) fn plain_systemctl(stderr: &str) -> String {
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
