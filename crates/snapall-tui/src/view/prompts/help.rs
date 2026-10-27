// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The help: what snapshots are, then the keys.

use crate::view::*;

/// The help. Page one teaches what snapall is about, for someone who knows nothing of it; page two
/// says what each key and button does.
pub(crate) fn help_spec(page: usize) -> ModalSpec {
    let (title, lines) = if page == 0 {
        (" What is this?  (1 of 2) ", concepts_page())
    } else {
        (" Keys and buttons  (2 of 2) ", keys_page())
    };
    ModalSpec {
        title: title.into(),
        lines,
        color: ACCENT,
        width: 84,
        height: 22,
        buttons: vec![confirm_button("Other page"), close_button()],
    }
}

pub(crate) fn concepts_page() -> Vec<Line<'static>> {
    let term = |name: &'static str, meaning: &'static str| {
        vec![
            Line::from(Span::styled(name, Style::new().fg(ACCENT).bold())),
            Line::from(format!("    {meaning}")),
        ]
    };
    let mut lines: Vec<Line> = explain::WHAT_IS_A_SNAPSHOT
        .split("\n\n")
        .flat_map(|paragraph| [Line::from(paragraph.to_owned()), Line::from("")])
        .collect();
    lines.extend(term(
        "Protected area (the list on the left)",
        "A part of your disk that snapshots are kept for, such as System or Home.",
    ));
    lines.extend(term(
        "Snapshot (the list on the right)",
        "One saved moment of that area. Newest first.",
    ));
    lines.extend(term("Restore", explain::RESTORING));
    lines
}

pub(crate) fn keys_page() -> Vec<Line<'static>> {
    let rows = [
        ("1  2  3", "Snapshots, Health, Activity"),
        ("up/down  k/j", "Move in the list that is selected"),
        ("left/right  Tab", "Switch between areas and snapshots"),
        ("space", "Mark a snapshot (Delete can take several)"),
        (
            "c  a",
            "Take a snapshot of this area, or of every one (Tab: a group)",
        ),
        ("d", "Delete marked snapshots for good (you type destroy)"),
        (
            "u  A",
            "Restore a snapshot; A: put its whole restore point back",
        ),
        ("p", "Protect: see what has no snapshots yet, and set it up"),
        (
            "g",
            "Groups: name a set of areas (theming) to snapshot together",
        ),
        ("R", "Restore points: put every area back to one moment"),
        (
            "x  U",
            "Activity: ask a change to stop. Health: stop protecting",
        ),
        ("r", "Look again at which snapshots exist"),
        (
            "y  destroy",
            "Answer a question: y for a change, destroy for a loss",
        ),
        ("?", "This help"),
        ("q  Ctrl-C", "Close snapall (changes nothing)"),
        (
            "mouse",
            "Click anything; hover a button to read what it does",
        ),
    ];
    rows.iter()
        .map(|(key, what)| {
            Line::from(vec![
                Span::styled(format!("{key:<20}"), Style::new().fg(ACCENT).bold()),
                Span::raw(*what),
            ])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{footer_buttons, MAX_PROMPT_HEIGHT};
    use ratatui::layout::Rect;

    fn words(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    /// What a prompt has of an 80-by-24 screen: its text rows, after the frame and the button row.
    const TEXT_ROWS: u16 = MAX_PROMPT_HEIGHT - 3;

    #[test]
    fn both_help_pages_fit_in_the_space_a_prompt_has_so_nothing_is_cut_off_the_end() {
        for (name, page) in [("concepts", concepts_page()), ("keys", keys_page())] {
            let needed = rows_needed(&page, 84 - 2);
            assert!(
                needed <= TEXT_ROWS,
                "the {name} page needs {needed} rows and a prompt has {TEXT_ROWS}: its last lines would be cut off"
            );
        }
    }

    #[test]
    fn every_button_on_the_bar_has_its_key_explained_in_the_help() {
        let keys = words(&keys_page());
        let screen = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 2,
        };
        for file_mode in [false, true] {
            for (_, button, letter, label) in footer_buttons(screen, file_mode) {
                // the key column is the first twenty cells: the letter is one of the keys listed there
                let explained = keys.iter().any(|line| {
                    let column: String = line.chars().take(20).collect();
                    column
                        .split_whitespace()
                        .any(|key| key == letter.to_string())
                });
                assert!(
                    explained,
                    "the `{letter} {label}` button is on the bar but the help's keys page never explains `{letter}`"
                );
                let _ = button;
            }
        }
    }
}
