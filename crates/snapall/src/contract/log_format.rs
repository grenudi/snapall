// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How the log is written: not at all, as text, or as JSON.

use argenv_resolve::FromRaw;

/// How events are written. `Off` is the default: a command-line tool is quiet unless asked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Off,
    Text,
    Json,
}

impl LogFormat {
    pub const TOKENS: &'static [&'static str] = &["off", "text", "json"];
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LogFormat::Off => "off",
            LogFormat::Text => "text",
            LogFormat::Json => "json",
        })
    }
}

impl FromRaw for LogFormat {
    fn from_raw(raw: &str) -> Option<LogFormat> {
        Some(match raw.trim() {
            "off" | "none" => LogFormat::Off,
            "text" => LogFormat::Text,
            "json" => LogFormat::Json,
            _ => return None,
        })
    }
}
