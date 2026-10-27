// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running the snapper program and what it leaves behind.

use std::io;
use std::process::Command;

pub(crate) const PROGRAM: &str = "snapper";

/// What a finished command left behind.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub success: bool,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a program with arguments. No shell is ever involved.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[String]) -> io::Result<CommandOutput>;
}

/// Runs real processes.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> io::Result<CommandOutput> {
        let out = Command::new(program).args(args).output()?;
        Ok(CommandOutput {
            success: out.status.success(),
            status: out.status.to_string(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}
