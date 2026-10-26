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
        ("c", "Take a snapshot of the selected area now"),
        ("a", "Take a snapshot of every protected area"),
        ("d", "Delete marked snapshots for good (files untouched)"),
        (
            "u",
            "Restore: go back to the selected snapshot (asks first)",
        ),
        ("A", "In Restore: put its whole restore point back"),
        ("p", "Protect: see what has no snapshots yet, and set it up"),
        (
            "g",
            "Groups: name a set of areas (theming) to snapshot together",
        ),
        ("R", "Restore points: put every area back to one moment"),
        ("x", "In Activity: ask a running change to stop"),
        (
            "U",
            "In Health: stop protecting an area (deletes its snapshots)",
        ),
        ("r", "Look again at which snapshots exist"),
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
