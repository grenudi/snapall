// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composition root and imperative shell: wires the argenv-driven CLI, the TUI and the
//! snapper adapter to the core, performs the I/O, and maps outcomes to exit codes.
//!
//! Exit status: 0 done; 1 a snapshot or snapper call failed; 2 the invocation is wrong.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod advice;
mod ask;
mod boot_probe;
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
use snapall_core::boot;
use snapall_core::{
    all_snapshots, config_covering, delete_snapshot, file_history, plan_snapshots, preview_restore,
    restore_snapshot, run_plan, snapshots_of, Observer, RestoreError, Silent, SnapshotBackend,
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

/// The subcommands as the generators present them: the very list `--help` prints.
fn generate_commands() -> Vec<argenv_resolve::generate::Command<'static>> {
    contract::Command::ALL
        .iter()
        .map(|c| argenv_resolve::generate::Command {
            name: c.token(),
            summary: c.summary(),
        })
        .collect()
}

/// What the completion and man page generators need to know that the contract does not say.
fn generated_program<'a>(
    commands: &'a [argenv_resolve::generate::Command<'a>],
    date: &'a str,
) -> argenv_resolve::generate::Program<'a> {
    argenv_resolve::generate::Program {
        name: "snapall",
        version: VERSION,
        about: "snapshot every snapper config in one command",
        commands,
        exit_status: &[
            ("0", "done"),
            (
                "1",
                "a snapshot or snapper call failed (or there is no terminal for tui)",
            ),
            ("2", "the invocation is wrong"),
            (
                "128+n",
                "tui was stopped by signal n; the terminal is restored first",
            ),
        ],
        see_also: &["snapper(8)", "snapper-configs(5)"],
        date,
    }
}

/// The man page date, `YYYY-MM-DD`, from `SOURCE_DATE_EPOCH` when given (so builds are
/// reproducible) and from the clock otherwise.
fn man_date(source_date_epoch: Option<u64>) -> String {
    let when = match source_date_epoch {
        Some(seconds) => std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
        None => std::time::SystemTime::now(),
    };
    snapall_observe::rfc3339_utc(when)
        .chars()
        .take(10)
        .collect()
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
/// `Send + Sync` so the observed backend can serve the TUI's worker thread.
fn build_observer(logging: &Logging) -> Result<Box<dyn Observer + Send + Sync>, Box<dyn Error>> {
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
        hints,
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
            if json {
                emit_json(&render::deleted_json(&config, id))?;
            } else {
                emit(&render::deleted(&config, id))?;
            }
        }
        Request::Changes { config, id, path } => {
            let preview = preview_restore(&backend, &config, id, path.as_deref())?;
            if json {
                emit_json(&render::changes_json(&config, id, &preview))?;
            } else {
                emit(&render::changes(&config, id, &preview))?;
            }
        }
        Request::Restore {
            config,
            id,
            path,
            dry_run,
            yes,
        } => {
            let preview = preview_restore(&backend, &config, id, path.as_deref())?;
            if preview.is_system_root && path.is_none() {
                // Not a usage error and not a crash: a deliberate refusal, with the way that works.
                let route = boot::restore_route(&boot::assess(&boot_probe::system()));
                return Err(format!(
                    "{}\n{route}",
                    RestoreError::SystemRoot {
                        config: config.clone()
                    }
                )
                .into());
            }
            if dry_run || preview.changes.is_empty() {
                if json {
                    emit_json(&render::changes_json(&config, id, &preview))?;
                } else {
                    emit(&render::changes(&config, id, &preview))?;
                }
                return Ok(ExitCode::SUCCESS);
            }
            if !yes {
                if !ask::can_ask() {
                    eprintln!(
                        "snapall: restore changes files and there is no terminal to ask on; \
                         look first with `snapall changes`, then pass --yes"
                    );
                    return Ok(ExitCode::from(2));
                }
                eprint!("{}", render::changes(&config, id, &preview));
                let count = preview.changes.len();
                let question = format!(
                    "Restore {count} path{} of {config} to #{id}? The current state is snapshotted first, so this can be undone.",
                    if count == 1 { "" } else { "s" }
                );
                if !ask::yes_no(&question)? {
                    eprintln!("snapall: not restoring");
                    return Ok(ExitCode::FAILURE);
                }
            }
            let done = restore_snapshot(&backend, &config, id, path.as_deref())?;
            if json {
                emit_json(&render::restored_json(&config, id, &done))?;
            } else {
                emit(&render::restored(&config, id, &done))?;
            }
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
            // A nudge, to a person, on stderr (stdout stays parseable), and only when it helps.
            if hints && !json && std::io::stderr().is_terminal() {
                if let Some(tip) = boot::hint(&boot::assess(&boot_probe::system())) {
                    eprintln!("{tip}");
                }
            }
        }
        Request::Doctor { install, yes } => {
            return doctor::run(&backend, &observer, install, yes, json);
        }
        Request::Completions { shell } => {
            let commands = generate_commands();
            let program = generated_program(&commands, "");
            emit(&argenv_resolve::generate::completions(
                shell,
                &program,
                &Contract::records(),
            )?)?;
        }
        Request::Man { source_date_epoch } => {
            let date = man_date(source_date_epoch);
            let commands = generate_commands();
            let program = generated_program(&commands, &date);
            emit(&argenv_resolve::generate::man_page(
                &program,
                &Contract::records(),
            )?)?;
        }
        Request::History { path } => {
            let path = absolute(&path)?;
            let found = file_history(&backend, &path).map_err(|e| advice::explain(&e, &path))?;
            if json {
                emit_json(&render::history_json(&found))?;
            } else {
                emit(&render::history(&found))?;
            }
        }
        Request::Tui { file } => {
            if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
                return Err("`snapall tui` needs an interactive terminal".into());
            }
            // Which config covers the file is settled here, before the terminal is taken over, so
            // that a file nothing covers is an error message and not a screen that opens empty.
            let target = match file {
                Some(named) => {
                    let path = absolute(&named)?;
                    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
                        return Err("directories are not supported yet: choose a file".into());
                    }
                    let config =
                        config_covering(&backend, &path).map_err(|e| advice::explain(&e, &path))?;
                    Some(snapall_tui::FileTarget { path, config })
                }
                None => None,
            };
            // Looked at once, here, because the UI's model must not read the machine itself.
            let status = boot::assess(&boot_probe::system());
            let context = snapall_tui::UiContext {
                boot_tip: if hints { boot::hint(&status) } else { None },
                restore_route: boot::restore_route(&status),
                file: target,
            };
            match snapall_tui::run(&backend, context)? {
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

/// The absolute path of what the user named. A relative path is taken from the working directory,
/// and symlinks in the *directory* part are resolved, so that a path reached through `/home` being
/// a link to `/var/home` is found in the subvolume that really holds it. The file's own name is
/// kept as given: a symlink is itself what is asked about, and a deleted file has nothing to resolve.
fn absolute(path: &str) -> Result<String, Box<dyn Error>> {
    let given = std::path::Path::new(path);
    let joined = if given.is_absolute() {
        given.to_owned()
    } else {
        std::env::current_dir()?.join(given)
    };
    let (Some(parent), Some(name)) = (joined.parent(), joined.file_name()) else {
        return Err(format!("`{path}` does not name a file").into());
    };
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("{}: {e}", parent.display()))?;
    parent
        .join(name)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| "a path must be valid UTF-8".into())
}
