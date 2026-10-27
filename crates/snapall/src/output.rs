// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Printing, and the choice between text for a person and JSON for a program.

use std::error::Error;
use std::io::{self, Write};
use std::process::ExitCode;

use serde_json::Value;

/// How a command ends: with an exit code, or with the error to print.
pub type Outcome = Result<ExitCode, Box<dyn Error>>;

/// Write to stdout. A reader that goes away (`snapall list | head`) is not an error.
pub fn emit(text: &str) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout().lock();
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.map_err(Into::into),
    }
}

/// Something said to the person on the way, apart from the result: it goes to stderr, so a result piped
/// to another program stays clean.
pub fn note(text: &str) {
    let _ = writeln!(io::stderr(), "{text}");
}

pub fn emit_json(value: &Value) -> Result<(), Box<dyn Error>> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    emit(&text)
}

/// How results are shown: as text for a person, or, with `--json`, as JSON for a program.
pub struct Show {
    json: bool,
}

impl Show {
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    pub fn is_json(&self) -> bool {
        self.json
    }

    /// A command's result, given in both forms; the one that was asked for is printed.
    pub fn result(&self, for_people: String, for_programs: Value) -> Outcome {
        if self.json {
            emit_json(&for_programs)?;
        } else {
            emit(&for_people)?;
        }
        Ok(ExitCode::SUCCESS)
    }

    /// Text that is the same whatever `--json` says: help, the version, a man page.
    pub fn text(&self, text: &str) -> Outcome {
        emit(text)?;
        Ok(ExitCode::SUCCESS)
    }

    /// A document that is JSON by nature, such as the contract.
    pub fn json(&self, value: &Value) -> Outcome {
        emit_json(value)?;
        Ok(ExitCode::SUCCESS)
    }
}
