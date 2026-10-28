// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The program: read what was asked, set up what it works with, carry it out, and end with the right exit code.
//!
//! Exit status: 0 done; 1 a snapshot or snapper call failed; 2 the invocation is wrong.

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

use crate::cli::Parsed;
use crate::commands::Context;
use crate::output::{Outcome, Show};

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
        Err(err) => ask::how_it_ended(err.as_ref()).unwrap_or_else(|| {
            eprintln!("snapall: {err}");
            ExitCode::FAILURE
        }),
    }
}

fn run(parsed: Parsed) -> Outcome {
    let Parsed {
        request,
        no_dbus,
        json,
        hints,
        logging,
        ..
    } = parsed;
    let observer = logging::observer(&logging)?;
    let backend = Observed::new(SnapperCli::system().with_no_dbus(no_dbus), &observer);
    let show = Show::new(json);
    let ctx = Context {
        backend: &backend,
        show: &show,
        observer: observer.as_ref(),
        hints,
        json,
        no_dbus,
    };
    commands::carry_out(&ctx, request)
}
