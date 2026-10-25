// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asking the person at the terminal a yes/no question, in one place.

use std::error::Error;
use std::io::{self, BufRead, IsTerminal, Write};

/// Whether there is a terminal to ask on.
pub fn can_ask() -> bool {
    io::stdin().is_terminal()
}

/// Ask `question` and read the answer from stdin; only `y` or `yes` is a yes. The caller checks
/// [`can_ask`] first, so that "no terminal" can be reported in its own words.
pub fn yes_no(question: &str) -> Result<bool, Box<dyn Error>> {
    eprint!("{question} [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// Like [`yes_no`], but Enter alone means `default`, and the prompt shows which: for questions where
/// one answer is the one snapall recommends.
pub fn confirm(question: &str, default: bool) -> Result<bool, Box<dyn Error>> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    eprint!("{question} {hint} ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(match answer.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    })
}
