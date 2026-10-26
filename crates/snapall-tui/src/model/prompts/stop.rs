// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asking before a change that is running is asked to stop: what stopping means, so that nobody is
//! afraid to do it, and that it is only a request.

use crate::model::*;

/// Enter or y asks; Esc leaves.
pub(crate) fn confirming_stop(state: &mut State, key: Key, id: String, what: String) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q' | 'n' | 'N') => Reaction::closes(),
        Key::Enter | Key::Char('y' | 'Y') => {
            state.notice = Some(Notice::Info(format!("asking it to stop: {what}")));
            Reaction::closes_and_starts(vec![Effect::StopChange { id }])
        }
        _ => Reaction::stays(Modal::StopChange { id, what }),
    }
}
