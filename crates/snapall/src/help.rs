// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `--help`, generated from the contract so it can never disagree with what is accepted.

use argenv_resolve::Record;

use crate::cli;

/// Width of the description column, so `--help` stays inside about 90 columns.
const SUMMARY_WIDTH: usize = 58;

/// Greedy word wrap. A word longer than `width` gets a line to itself.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

pub fn render(records: &[Record], version: &str) -> String {
    let mut out = format!(
        "snapall {version}\nSnapshot every protected area in one command.\n\n\
         USAGE\n    snapall [OPTIONS] [DESCRIPTION]    shorthand for `snapall create`\n    \
         snapall <COMMAND> [OPTIONS]\n\nCOMMANDS\n"
    );
    for (token, summary) in cli::commands() {
        out.push_str(&format!("    {token:<10} {summary}\n"));
    }

    // Groups in the order the contract first mentions them.
    let mut groups: Vec<&str> = Vec::new();
    for record in records {
        let group = record.group.as_deref().unwrap_or("options");
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    for group in groups {
        out.push_str(&format!("\n{}\n", group.to_uppercase()));
        for record in records
            .iter()
            .filter(|r| r.group.as_deref().unwrap_or("options") == group)
        {
            let usage = record.usage();
            let usage = if usage.is_empty() {
                record.env_names().join(", ")
            } else {
                usage
            };
            let summary = record.summary.as_deref().unwrap_or("");
            let mut lines = wrap(summary, SUMMARY_WIDTH).into_iter();
            if usage.chars().count() <= 26 {
                out.push_str(&format!(
                    "    {usage:<26} {}\n",
                    lines.next().unwrap_or_default()
                ));
            } else {
                out.push_str(&format!("    {usage}\n"));
                out.push_str(&format!(
                    "    {:<26} {}\n",
                    "",
                    lines.next().unwrap_or_default()
                ));
            }
            for line in lines {
                out.push_str(&format!("    {:<26} {line}\n", ""));
            }
            let mut notes = Vec::new();
            if !record.env_names().is_empty() && record.arg.is_some() {
                notes.push(format!("env: {}", record.env_names().join(", ")));
            }
            if !record.subcommands.is_empty() {
                notes.push(format!("commands: {}", record.subcommands.join(", ")));
            }
            if let Some(example) = &record.example {
                notes.push(format!("e.g. {example}"));
            }
            if !notes.is_empty() {
                for line in wrap(&format!("({})", notes.join("; ")), SUMMARY_WIDTH) {
                    out.push_str(&format!("    {:<26} {line}\n", ""));
                }
            }
        }
    }
    out.push_str(
        "\nEXIT STATUS\n    0  done    1  a snapshot or snapper call failed    2  the invocation is wrong\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::commands;
    use crate::contract::Contract;

    #[test]
    fn wrap_breaks_on_words_and_keeps_long_words_whole() {
        assert_eq!(wrap("aa bb cc", 5), ["aa bb", "cc"]);
        assert_eq!(wrap("abcdefghij kl", 4), ["abcdefghij", "kl"]);
        assert!(wrap("", 10).is_empty());
    }

    #[test]
    fn help_is_generated_from_the_contract_and_stays_narrow() {
        let text = render(&Contract::records(), "9.9.9");
        assert!(text.starts_with("snapall 9.9.9\n"));
        for (token, summary) in commands() {
            assert!(text.contains(token) && text.contains(summary), "{token}");
        }
        for record in Contract::records() {
            let shown = record.usage();
            let shown = if shown.is_empty() {
                record.env_names().join(", ")
            } else {
                shown
            };
            assert!(text.contains(&shown), "help should mention `{shown}`");
            for env in record.env_names() {
                assert!(text.contains(env), "help should mention {env}");
            }
        }
        let widest = text.lines().map(|l| l.chars().count()).max().unwrap();
        assert!(widest <= 95, "help is {widest} columns wide:\n{text}");
    }
}
