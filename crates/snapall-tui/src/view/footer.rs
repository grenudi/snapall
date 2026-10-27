// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two lines under the status bar: the explanation of whatever the mouse is over, and the keys as buttons.

use crate::view::*;

/// The footer's buttons: the button, the key it stands for, its label, and which of the two rows it is on.
/// The first row is for doing things, the second for finding your way and leaving.
pub(crate) const ACTIONS: [(Button, &str, &str, u16); 10] = [
    (Button::Create, "c", "create", 0),
    (Button::CreateAll, "a", "all", 0),
    (Button::Delete, "d", "delete", 0),
    (Button::Restore, "u", "restore", 0),
    (Button::Groups, "g", "groups", 0),
    (Button::Points, "R", "points", 1),
    (Button::Reload, "r", "reload", 1),
    (Button::Protect, "p", "protect", 1),
    (Button::Help, "?", "help", 1),
    (Button::Quit, "q", "quit", 1),
];

/// The footer of the screen that is open on one file.
pub(crate) const FILE_ACTIONS: [(Button, &str, &str, u16); 4] = [
    (Button::Restore, "u", "restore this version", 0),
    (Button::Reload, "r", "reload", 0),
    (Button::Help, "?", "help", 0),
    (Button::Quit, "q", "quit", 0),
];

/// Whether a button does anything where the person is: taking, deleting and restoring act on the snapshot
/// lists, so they have nothing to act on in the other tabs. They are drawn dim and do not answer the mouse,
/// rather than looking alive and doing nothing.
pub(crate) fn is_disabled(state: &State, button: Button) -> bool {
    matches!(button, Button::Create | Button::Delete | Button::Restore)
        && state.file.is_none()
        && state.tab != Tab::Snapshots
}

/// The cells a button takes: a space, the key, a space, the label and a space. The key is the chip, so the
/// text still reads `c create`.
fn width_of(key: &str, label: &str) -> u16 {
    u16::try_from(key.chars().count() + label.chars().count() + 3).unwrap_or(u16::MAX)
}

/// Each footer button and the cells it covers.
pub(crate) fn footer_buttons(
    keys: Rect,
    file_mode: bool,
) -> Vec<(Rect, Button, &'static str, &'static str)> {
    let actions: &[(Button, &str, &str, u16)] = if file_mode { &FILE_ACTIONS } else { &ACTIONS };
    let mut next_x = [keys.x.saturating_add(1); 2];
    let mut out = Vec::new();
    for &(button, key, label, row) in actions {
        let width = width_of(key, label);
        let slot = usize::from(row.min(1));
        if row >= keys.height
            || next_x[slot].saturating_add(width) > keys.x.saturating_add(keys.width)
        {
            continue; // what does not fit is not drawn, and so cannot be clicked
        }
        out.push((
            Rect {
                x: next_x[slot],
                y: keys.y.saturating_add(row),
                width,
                height: 1,
            },
            button,
            key,
            label,
        ));
        next_x[slot] = next_x[slot].saturating_add(width).saturating_add(1);
    }
    out
}

/// What a button does, in plain words, for the footer line while the mouse is over it. The same
/// button means different things in different prompts, so the prompt it is in is taken into account.
pub fn button_help(button: Button, state: &State) -> &'static str {
    match (&state.modal, button) {
        (Some(Modal::Describe { .. }), Button::Confirm) => "Take the snapshot now",
        (Some(Modal::ConfirmDelete { .. }), Button::Confirm) => {
            "Delete them for good (your files are not touched)"
        }
        (Some(Modal::Restore { .. }), Button::Confirm) => {
            "Put the files back (how they are now is saved first, so this can be undone)"
        }
        (Some(Modal::Help { .. }), Button::Confirm) => "Show the other page",
        (Some(Modal::Protect { .. }), Button::Confirm) => {
            "Set this part of the disk up for snapshots (nothing is copied or moved)"
        }
        (Some(Modal::Protect { .. }), Button::Next) => "Show the next part of the disk",
        (Some(Modal::Points { .. }), Button::Confirm) => {
            "Put every area of the chosen restore point back (not the system)"
        }
        (Some(Modal::Restore { .. }), Button::RestoreAll) => {
            "Put all areas of this restore point back at once (not the system)"
        }
        (Some(_), Button::Cancel) => "Close this without changing anything",
        (Some(_), _) => "",
        (None, Button::Create) => {
            "Take a snapshot of the selected area now: a saved moment you can go back to"
        }
        (None, Button::CreateAll) => "Take a snapshot of every protected area at once",
        (None, Button::Delete) => {
            "Delete the marked snapshots for good. Your files are not touched"
        }
        (None, Button::Restore) if state.file.is_some() => {
            "Put this file back to how this snapshot had it (asks first)"
        }
        (None, Button::Restore) => {
            "Go back to the selected snapshot: shows what changes and asks first"
        }
        (None, Button::Reload) => "Look again at which snapshots exist",
        (None, Button::Points) => "See the restore points and put every area back to one",
        (None, Button::Protect) => {
            "See which parts of your disk have no snapshots yet, and protect them"
        }
        (None, Button::Help) => "Explain what snapshots are and what each key does",
        (None, Button::Quit) => "Close snapall. Nothing is changed",
        (_, Button::Groups) => "Name sets of your areas, such as theming, and snapshot just those",
        (None, Button::Confirm | Button::Cancel | Button::Next | Button::RestoreAll) => "",
    }
}

/// What is being done and for how long; after a few seconds, that it can take a while and that waiting
/// is safe. `None` when nothing that changes anything is in flight.
pub(crate) fn busy_line(state: &State) -> Option<String> {
    let doing = state.doing.first()?;
    let seconds = state.busy_ticks / 10;
    let lead = if state.quit {
        "Closing as soon as this has finished: "
    } else {
        ""
    };
    Some(if seconds >= 5 {
        format!("{lead}This can take a while; waiting is safe. {doing} ({seconds} s)")
    } else {
        format!("{lead}{doing}... {seconds} s")
    })
}

pub(crate) fn render_footer(state: &State, frame: &mut Frame, areas: &Areas) {
    let explanation = state
        .hover
        .and_then(|over| match over {
            Click::Button(button) => Some(button_help(button, state)),
            Click::Tab(tab) => Some(tab.purpose()),
            _ => None,
        })
        .filter(|text| !text.is_empty());
    // What deserves attention comes before a tip: a part of the disk without snapshots, then the boot menu.
    let attention = state
        .context
        .protect_hint
        .as_ref()
        .filter(|_| state.offers.iter().any(|offer| offer.recommended))
        .or(state.context.boot_tip.as_ref());
    let notice = match (explanation, &state.notice, attention) {
        // Under the mouse: what the button does.
        (Some(text), _, _) => Span::styled(format!(" {text}"), Style::new().fg(Color::Cyan)),
        (None, Some(Notice::Error(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Red))
        }
        // Work in flight: what is being done and for how long, and after a while that waiting is safe.
        (None, _, _) if busy_line(state).is_some() => Span::styled(
            format!(" {}", busy_line(state).unwrap_or_default()),
            Style::new().fg(Color::Yellow),
        ),
        (None, Some(Notice::Info(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Green))
        }
        // The tip stays until something else needs the line.
        (None, None, Some(tip)) => Span::styled(format!(" {tip}"), Style::new().fg(Color::Yellow)),
        (None, None, None) => Span::raw(""),
    };
    frame.render_widget(Paragraph::new(Line::from(notice)), areas.notice);
    for (rect, button, key, label) in footer_buttons(areas.keys, state.file.is_some()) {
        let mood = if is_disabled(state, button) {
            look::Mood::Disabled
        } else {
            look::mood(state, Click::Button(button))
        };
        let danger = button == Button::Delete;
        let line = Line::from(vec![
            Span::styled(format!(" {key}"), look::chip(mood, danger)),
            Span::styled(format!(" {label} "), look::label(mood, danger)),
        ]);
        frame.render_widget(Paragraph::new(line), rect);
    }
}
