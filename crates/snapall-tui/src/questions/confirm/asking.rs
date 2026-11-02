// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The question before a change: what it will do, and the answer that goes with its weight. Every change
//! goes through here, so no prompt can forget to ask, and the answer is the one the table in the core assigns:
//! `y` or `yes` for a change, `destroy` (and the name, where there is one) for what is lost for good. A bare
//! Enter is never an answer: it is what a hand does by itself.

use crate::model::*;
use snapall_core::Act;

/// The question, as a prompt to open.
pub(crate) fn confirm_modal(
    act: Act,
    target: Option<&str>,
    lines: Vec<String>,
    then: Vec<Effect>,
) -> Modal {
    Modal::Confirm(Confirm {
        act,
        target: target.map(str::to_owned),
        lines,
        typed: String::new(),
        error: None,
        then,
        on_cancel: None,
        on_done: None,
    })
}

/// Ask first: what is not asked about (reading, taking a snapshot) starts at once, anything else opens the
/// question and starts only when it is answered.
pub(crate) fn ask_first(
    act: Act,
    target: Option<&str>,
    lines: Vec<String>,
    then: Vec<Effect>,
) -> Reaction {
    if !act.weight().asks() {
        return Reaction::closes_and_starts(then);
    }
    Reaction::stays(confirm_modal(act, target, lines, then))
}

/// [`ask_first`], for a prompt that wants to be where it was after the question, answered or not.
pub(crate) fn ask_first_then_return(
    act: Act,
    lines: Vec<String>,
    then: Vec<Effect>,
    back: Modal,
) -> Reaction {
    let Modal::Confirm(mut question) = confirm_modal(act, None, lines, then) else {
        unreachable!("confirm_modal makes a confirmation")
    };
    question.on_cancel = Some(Box::new(back.clone()));
    question.on_done = Some(Box::new(back));
    Reaction::stays(Modal::Confirm(question))
}

pub(crate) fn confirming(state: &mut State, key: Key, mut question: Confirm) -> Reaction {
    let weight = question.act.weight();
    match key {
        Key::Esc | Key::CtrlC => match question.on_cancel {
            Some(back) => Reaction::stays(*back),
            None => {
                state.notice = Some(Notice::Info("Nothing was changed".into()));
                Reaction::closes()
            }
        },
        Key::Backspace => {
            question.typed.pop();
            question.error = None;
            Reaction::stays(Modal::Confirm(question))
        }
        Key::Char(c) if !c.is_control() => {
            if question.typed.chars().count() < 64 {
                question.typed.push(c);
            }
            question.error = None;
            Reaction::stays(Modal::Confirm(question))
        }
        Key::Enter if weight.accepts(&question.typed, question.target.as_deref()) => {
            match question.on_done {
                Some(back) => Reaction::stays_and_starts(*back, question.then),
                None => Reaction::closes_and_starts(question.then),
            }
        }
        Key::Enter => {
            question.error = Some(weight.instruction(question.target.as_deref()));
            Reaction::stays(Modal::Confirm(question))
        }
        _ => Reaction::stays(Modal::Confirm(question)),
    }
}
