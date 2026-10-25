// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the audit trail goes: nowhere, to stderr, or to a private file.

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use snapall_core::{Observer, Silent};
use snapall_observe::{Format, Lines};

use crate::cli::Logging;
use crate::contract::LogFormat;

/// The observer that reports every operation as the user asked: not at all, as text, or as JSON,
/// to stderr or to a file.
pub fn observer(logging: &Logging) -> Result<Box<dyn Observer + Send + Sync>, Box<dyn Error>> {
    let format = match logging.format {
        LogFormat::Off => return Ok(Box::new(Silent)),
        LogFormat::Text => Format::Text,
        LogFormat::Json => Format::Json,
    };
    Ok(match &logging.file {
        Some(path) => Box::new(Lines::new(open_private_log(path)?, format)),
        None => Box::new(Lines::new(io::stderr(), format)),
    })
}

/// A log file others cannot read (0600), appended to, created if needed.
fn open_private_log(path: &Path) -> Result<File, Box<dyn Error>> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("cannot open the log file {}: {e}", path.display()).into())
}
