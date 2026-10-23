// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composition root and imperative shell: wires the argenv-driven CLI, the TUI and the
//! snapper adapter to the core, performs the I/O, and maps outcomes to exit codes.
//!
//! Exit status: 0 done; 1 a snapshot or snapper call failed; 2 the invocation is wrong.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod cli;
mod contract;
mod doctor;
mod help;
mod render;

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::ExitCode;

use argenv_resolve::ProcessEnv;
use serde_json::Value;
use snapall_core::{
    all_snapshots, delete_snapshot, plan_snapshots, run_plan, snapshots_of, Observer, Silent,
    SnapshotBackend,
};
use snapall_observe::{Format, Lines, Observed};
use snapall_snapper::SnapperCli;

use crate::cli::{Logging, Parsed, Request};
use crate::contract::{Contract, LogFormat};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match cli::parse(&args, &ProcessEnv) {
        Ok(parsed) => parsed,
        Err(usage) => {
            for line in &usage.0 {
                eprintln!("snapall: error: {line}");
            }
            eprintln!("Try `snapall --help`.");
            return ExitCode::from(2);
        }
    };
    for warning in &parsed.warnings {
        eprintln!("snapall: warning: {warning}");
    }
    match run(parsed) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("snapall: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Write to stdout. A reader that goes away (`snapall list | head`) is not an error: the
/// output was not wanted any more, which is how every well-behaved filter ends. (Rust ignores
/// SIGPIPE, so `print!` would panic with "Broken pipe" instead.)
fn emit(text: &str) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout().lock();
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.map_err(Into::into),
    }
}

fn emit_json(value: &Value) -> Result<(), Box<dyn Error>> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    emit(&text)
}

/// A log file is appended to and created private (0600): it records what was done to the system.
fn open_log_file(path: &Path) -> Result<File, Box<dyn Error>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("cannot open the log file {}: {e}", path.display()).into())
}

/// Where events go: nowhere, stderr (an event stream the environment routes), or a file.
fn build_observer(logging: &Logging) -> Result<Box<dyn Observer>, Box<dyn Error>> {
    let format = match logging.format {
        LogFormat::Off => return Ok(Box::new(Silent)),
        LogFormat::Text => Format::Text,
        LogFormat::Json => Format::Json,
    };
    Ok(match &logging.file {
        Some(path) => Box::new(Lines::new(open_log_file(path)?, format)),
        None => Box::new(Lines::new(io::stderr(), format)),
    })
}

fn run(parsed: Parsed) -> Result<ExitCode, Box<dyn Error>> {
    let Parsed {
        request,
        no_dbus,
        json,
        logging,
        ..
    } = parsed;
    // Everything that touches snapper goes through the observed backend, so every operation is
    // reported without any use case knowing about logging.
    let observer = build_observer(&logging)?;
    let backend = Observed::new(SnapperCli::system().with_no_dbus(no_dbus), &observer);

    match request {
        Request::Help => {
            emit(&help::render(&Contract::records(), VERSION))?;
        }
        Request::Version => emit(&format!("snapall {VERSION}\n"))?,
        Request::Contract => {
            let source = format!("snapall@{VERSION}");
            emit_json(&argenv_resolve::document(&source, &Contract::records()))?;
        }
        Request::List => {
            let configs = backend.list_configs()?;
            if json {
                emit_json(&render::configs_json(&configs))?;
            } else {
                emit(&render::configs(&configs))?;
            }
        }
        Request::Snapshots { config } => {
            let all = match config {
                Some(config) => {
                    let list = snapshots_of(&backend, &config)?;
                    vec![(config, list)]
                }
                None => all_snapshots(&backend)?,
            };
            if json {
                emit_json(&render::snapshots_json(&all))?;
            } else {
                emit(&render::snapshots(&all))?;
            }
        }
        Request::Delete { config, id } => {
            delete_snapshot(&backend, &config, id)?;
            emit(&format!("deleted #{id} from {config}\n"))?;
        }
        Request::Create {
            only,
            description,
            dry_run,
        } => {
            let plan = plan_snapshots(&backend, only.as_deref())?;
            if dry_run {
                if json {
                    emit_json(&render::dry_run_json(&plan, &description))?;
                } else {
                    emit(&render::dry_run(&plan, &description))?;
                }
                return Ok(ExitCode::SUCCESS);
            }
            let report = run_plan(&backend, &plan, &description);
            if json {
                emit_json(&render::report_json(&report))?;
            } else {
                emit(&render::report(&report))?;
            }
            if !report.is_success() {
                return Ok(ExitCode::FAILURE);
            }
        }
        Request::Doctor { install, yes } => {
            return doctor::run(&backend, &observer, install, yes, json);
        }
        Request::Tui => {
            if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
                return Err("`snapall tui` needs an interactive terminal".into());
            }
            match snapall_tui::run(&backend)? {
                snapall_tui::Exit::Quit => {}
                // Exit as a signalled process would, once the terminal is back to normal.
                snapall_tui::Exit::Signal(signal) => {
                    return Ok(ExitCode::from(u8::try_from(128 + signal).unwrap_or(1)));
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
