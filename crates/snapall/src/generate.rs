// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell completions and the man page. Neither is written by hand: both come from the contract,
//! so they cannot disagree with what the program accepts.

use argenv_resolve::generate::{self, Command, Program, Shell};

use crate::contract::{self, Contract};
use crate::output::{Outcome, Show};
use crate::VERSION;

pub fn completions(show: &Show, shell: Shell) -> Outcome {
    let commands = commands();
    let program = program(&commands, "");
    show.text(&generate::completions(
        shell,
        &program,
        &Contract::records(),
    )?)
}

pub fn man_page(show: &Show, source_date_epoch: Option<u64>) -> Outcome {
    let date = man_date(source_date_epoch);
    let commands = commands();
    let program = program(&commands, &date);
    show.text(&generate::man_page(&program, &Contract::records())?)
}

fn commands() -> Vec<Command<'static>> {
    contract::Command::ALL
        .iter()
        .map(|c| Command {
            name: c.token(),
            summary: c.summary(),
        })
        .collect()
}

fn program<'a>(commands: &'a [Command<'a>], date: &'a str) -> Program<'a> {
    Program {
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

/// The date on the man page: `SOURCE_DATE_EPOCH` when it is set, so that a build is reproducible.
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
