// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Which version of snapper is installed, or why that could not be found out.

use crate::running::PROGRAM;
use std::process::Command;

pub fn installed_version() -> Result<String, String> {
    match Command::new(PROGRAM).arg("--version").output() {
        Ok(out) if out.status.success() => Ok(first_line(&out.stdout)),
        Ok(out) => {
            let why = first_line(&out.stderr);
            Err(if why.is_empty() {
                out.status.to_string()
            } else {
                why
            })
        }
        Err(error) => Err(error.to_string()),
    }
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_line_of_a_command_output_is_used() {
        assert_eq!(
            first_line(b"snapper 0.13.1\nlibsnapper 7.2.0\n"),
            "snapper 0.13.1"
        );
        assert_eq!(first_line(b""), "");
    }
}
