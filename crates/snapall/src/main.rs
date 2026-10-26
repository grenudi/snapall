// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composition root and imperative shell: wires the argenv-driven CLI, the TUI and the
//! snapper adapter to the core, performs the I/O, and maps outcomes to exit codes.
//!
//! Exit status: 0 done; 1 a snapshot or snapper call failed; 2 the invocation is wrong.
//!
//! Reading order: this file says *which* command runs; [`commands`] says what each one does;
//! `snapall-core` holds the rules they apply.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod advice;
mod ask;
mod boot_probe;
mod cli;
mod commands;
mod contract;
mod doctor;
mod generate;
mod help;
mod logging;
mod output;
mod render;
mod setup;

use std::process::ExitCode;

use argenv_resolve::ProcessEnv;
use snapall_observe::Observed;
use snapall_snapper::SnapperCli;

use crate::cli::{Parsed, Request};
use crate::commands::Context;
use crate::contract::Contract;
use crate::output::{Outcome, Show};

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

/// Run the command that was asked for.
fn run(parsed: Parsed) -> Outcome {
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
    let observer = logging::observer(&logging)?;
    let backend = Observed::new(SnapperCli::system().with_no_dbus(no_dbus), &observer);
    let show = Show::new(json);
    let ctx = Context {
        backend: &backend,
        show: &show,
        hints,
    };

    match request {
        // about the program
        Request::Help => show.text(&help::render(&Contract::records(), VERSION)),
        Request::Version => show.text(&format!("snapall {VERSION}\n")),
        Request::Contract => show.json(&argenv_resolve::document(
            &format!("snapall@{VERSION}"),
            &Contract::records(),
        )),
        Request::Completions { shell } => generate::completions(&show, shell),
        Request::Man { source_date_epoch } => generate::man_page(&show, source_date_epoch),

        // looking
        Request::List => commands::list(&ctx),
        Request::Points => commands::points(&ctx),
        Request::Health => commands::health(&ctx),
        Request::Activity => commands::activity(&ctx),
        Request::Cancel { job } => commands::cancel(&ctx, &job),
        Request::Unprotect { area, dry_run, yes } => commands::unprotect(&ctx, &area, dry_run, yes),
        Request::Limits {
            area,
            keep,
            min_age,
            cleanup,
            yes,
        } => commands::limits(
            &ctx,
            &area,
            commands::LimitsWanted {
                keep: keep.as_deref(),
                min_age: min_age.as_deref(),
                cleanup: cleanup.as_deref(),
            },
            yes,
        ),
        Request::Groups {
            action,
            name,
            areas,
        } => commands::groups(&ctx, action.as_deref(), name.as_deref(), areas.as_deref()),
        Request::Quotas { state, yes } => commands::quotas(&ctx, state.as_deref(), yes),
        Request::Snapshots { config } => commands::snapshots(&ctx, config),
        Request::History { path } => commands::history(&ctx, &path),
        Request::Changes { config, id, path } => {
            commands::changes(&ctx, &config, id, path.as_deref())
        }

        // changing
        Request::Create {
            only,
            description,
            dry_run,
            job,
            group,
        } => commands::create(
            &ctx,
            only.as_deref(),
            group.as_deref(),
            &description,
            dry_run,
            job.as_deref(),
        ),
        Request::Delete { config, id } => commands::delete(&ctx, &config, id),
        Request::Restore {
            config,
            id,
            path,
            dry_run,
            yes,
        } => commands::restore(&ctx, &config, id, path.as_deref(), dry_run, yes),

        Request::RestoreAll {
            point,
            yes,
            dry_run,
            job,
        } => commands::restore_all(&ctx, &point, dry_run, yes, job.as_deref()),

        // setting up
        Request::Setup { area, yes, dry_run } => {
            commands::setup(&ctx, area.as_deref(), yes, dry_run)
        }

        // helping and showing
        Request::Doctor { install, yes } => doctor::run(&backend, &observer, install, yes, json),
        Request::Tui { file } => commands::tui(&ctx, file.as_deref()),
    }
}
