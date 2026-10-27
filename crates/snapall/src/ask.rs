// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asking the person at the terminal a yes/no question, in one place.

use std::error::Error;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

use snapall_core::Weight;

/// Whether there is a terminal to ask on.
pub fn can_ask() -> bool {
    io::stdin().is_terminal()
}

/// What came of asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Go,
    Declined,
    /// It needed an answer and there is nobody to type one.
    Nobody,
}

/// Read one line from stdin.
fn read_line() -> Result<String, Box<dyn Error>> {
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(answer)
}

/// Ask what the weight of the thing requires, and nothing less: `y` or `yes` for a change, `destroy` (and
/// the name, where there is one) for what loses something for good, never a bare Enter. Said already by a
/// flag (`said_so`), it is not asked again. The caller reports [`Decision::Nobody`] in its own words.
pub fn decide(
    weight: Weight,
    said_so: bool,
    question: &str,
    target: Option<&str>,
) -> Result<Decision, Box<dyn Error>> {
    if !weight.asks() || said_so {
        return Ok(Decision::Go);
    }
    if !can_ask() {
        return Ok(Decision::Nobody);
    }
    eprint!("{question}\n{}: ", weight.instruction(target));
    Ok(if weight.accepts(&read_line()?, target) {
        Decision::Go
    } else {
        Decision::Declined
    })
}

pub fn before(
    act: snapall_core::Act,
    said_so: bool,
    question: &str,
) -> Result<Option<ExitCode>, Box<dyn Error>> {
    gate(act.weight(), said_so, question, None)
}

/// [`decide`], and if the answer is not a go, what to say and how to leave: `Some(exit)` means stop.
pub fn gate(
    weight: Weight,
    said_so: bool,
    question: &str,
    target: Option<&str>,
) -> Result<Option<ExitCode>, Box<dyn Error>> {
    Ok(match decide(weight, said_so, question, target)? {
        Decision::Go => None,
        Decision::Declined => {
            eprintln!("snapall: nothing changed");
            Some(ExitCode::FAILURE)
        }
        Decision::Nobody => {
            let flag = weight.script_flag().unwrap_or("--yes");
            eprintln!("snapall: this needs an answer and there is no terminal to ask on: read the above, then pass {flag}");
            Some(ExitCode::from(2))
        }
    })
}

/// A yes or no for a change, typed: only `y` or `yes` is a yes, and Enter alone is a no. For callers that
/// have already checked [`can_ask`].
pub fn yes_no(question: &str) -> Result<bool, Box<dyn Error>> {
    eprint!("{question}\n{}: ", Weight::Changes.instruction(None));
    Ok(Weight::Changes.accepts(&read_line()?, None))
}
