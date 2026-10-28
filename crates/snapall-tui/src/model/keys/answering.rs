// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a prompt answers a key: what it returns, and which prompt a key goes to.

use crate::model::*;

/// What a prompt becomes after a key, and the work that key starts.
pub(crate) struct Reaction {
    /// The prompt as it now is, or `None` if it closes.
    pub(crate) modal: Option<Modal>,
    pub(crate) effects: Vec<Effect>,
}

impl Reaction {
    /// The prompt stays open, as given (possibly changed by the key).
    pub(crate) fn stays(modal: Modal) -> Self {
        Self {
            modal: Some(modal),
            effects: Vec::new(),
        }
    }

    pub(crate) fn closes() -> Self {
        Self {
            modal: None,
            effects: Vec::new(),
        }
    }

    /// The prompt stays open and work starts (moving to another restore point looks it up).
    pub(crate) fn stays_and_starts(modal: Modal, effects: Vec<Effect>) -> Self {
        Self {
            modal: Some(modal),
            effects,
        }
    }

    /// The prompt is answered: it closes and the work it asked for starts.
    pub(crate) fn closes_and_starts(effects: Vec<Effect>) -> Self {
        Self {
            modal: None,
            effects,
        }
    }
}

pub(crate) fn on_modal_key(state: &mut State, key: Key) -> Vec<Effect> {
    let Some(modal) = state.modal.take() else {
        return Vec::new();
    };
    let reaction = match modal {
        Modal::Help { page } => reading_help(key, page),
        Modal::Protect { index } => protecting(state, key, index),
        Modal::Points {
            index,
            points,
            preview,
        } => browsing_points(key, index, points, preview),
        Modal::Job { unit } => controlling_job(state, key, unit),
        Modal::Quotas { turn_on } => confirming_quotas(state, key, turn_on),
        Modal::StopChange { id, what } => confirming_stop(state, key, id, what),
        Modal::Groups(screen) => managing_groups(state, key, screen),
        Modal::Limits(editor) => editing_limits(state, key, editor),
        Modal::StopProtecting(prompt) => confirming_unprotect(state, key, prompt),
        Modal::Confirm(question) => confirming(state, key, question),
        Modal::Describe { target, input, .. } => describing(state, key, target, input),
        Modal::ConfirmDelete { area, ids } => confirming_delete(state, key, area, ids),
        Modal::Restore {
            area,
            id,
            path,
            stage,
            offer,
        } => confirming_restore(state, key, area, id, path, stage, offer),
    };
    state.modal = reaction.modal;
    reaction.effects
}
