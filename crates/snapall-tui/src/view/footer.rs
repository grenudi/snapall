// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two lines under the status bar: the explanation of whatever the mouse is over, and the keys as buttons.

use crate::view::*;

/// The footer's buttons, in order: the button, the key it stands for, and its label.
pub(crate) const ACTIONS: [(Button, &str, &str); 9] = [
    (Button::Create, "c", "create"),
    (Button::CreateAll, "a", "all"),
    (Button::Delete, "d", "delete"),
    (Button::Restore, "u", "restore"),
    (Button::Points, "R", "points"),
    (Button::Reload, "r", "reload"),
    (Button::Protect, "p", "protect"),
    (Button::Help, "?", "help"),
    (Button::Quit, "q", "quit"),
];

/// Each footer button and the cells it covers on the key line.
pub(crate) const FILE_ACTIONS: [(Button, &str, &str); 4] = [
    (Button::Restore, "u", "restore this version"),
    (Button::Reload, "r", "reload"),
    (Button::Help, "?", "help"),
    (Button::Quit, "q", "quit"),
];

pub(crate) fn footer_buttons(
    keys: Rect,
    file_mode: bool,
) -> Vec<(Rect, Button, &'static str, &'static str)> {
    let mut x = keys.x.saturating_add(1);
    let mut out = Vec::new();
    let actions: &[(Button, &str, &str)] = if file_mode { &FILE_ACTIONS } else { &ACTIONS };
    for &(button, key, label) in actions {
        let width = (key.chars().count() + 1 + label.chars().count()) as u16;
        if x.saturating_add(width) > keys.x.saturating_add(keys.width) {
            break; // what does not fit is not drawn, and so cannot be clicked
        }
        out.push((
            Rect {
                x,
                y: keys.y,
                width,
                height: 1.min(keys.height),
            },
            button,
            key,
            label,
        ));
        // one space between buttons: nine of them must fit 80 columns, and the key letter is bold
        x = x.saturating_add(width).saturating_add(1);
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
        (None, Button::Confirm | Button::Cancel | Button::Next | Button::RestoreAll) => "",
    }
}

pub(crate) fn render_footer(state: &State, frame: &mut Frame, areas: &Areas) {
    let explanation = state
        .hover
        .map(|button| button_help(button, state))
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
        (None, Some(Notice::Info(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Green))
        }
        (None, Some(Notice::Error(text)), _) => {
            Span::styled(format!(" {text}"), Style::new().fg(Color::Red))
        }
        // The tip stays until something else needs the line.
        (None, None, Some(tip)) => Span::styled(format!(" {tip}"), Style::new().fg(Color::Yellow)),
        (None, None, None) => Span::raw(""),
    };
    frame.render_widget(Paragraph::new(Line::from(notice)), areas.notice);
    for (rect, _button, key, label) in footer_buttons(areas.keys, state.file.is_some()) {
        let line = Line::from(vec![
            Span::styled(key, Style::new().fg(ACCENT).bold()),
            Span::styled(format!(" {label}"), Style::new().fg(DIM)),
        ]);
        frame.render_widget(Paragraph::new(line), rect);
    }
}
