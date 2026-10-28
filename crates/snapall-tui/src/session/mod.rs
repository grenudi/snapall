// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window run synchronously: the model plus an executor, as the tests drive it.
//!
//! [`Executor`] performs one effect against a backend and answers with the message that describes the outcome.
//! [`Session`] drives the model with an executor: every effect runs at once and its result is fed back before the
//! next key. The real terminal (`term.rs`) uses the same executor on a worker thread instead, so that a slow call
//! does not freeze the screen.

mod changing;
mod executor;
mod looking;

pub use executor::Executor;

use crate::model::{update, Key, Msg, State, UiContext};
use snapall_core::{SnapshotBackend, SystemProbe};
use std::collections::VecDeque;

/// The model plus an executor, run synchronously. Used by the tests.
pub struct Session<'a> {
    pub state: State,
    executor: Executor<'a>,
}

impl<'a> Session<'a> {
    /// Start a session and load the areas.
    pub fn new(backend: &'a (dyn SnapshotBackend + Sync)) -> Self {
        Self::with_context(backend, UiContext::default())
    }

    /// A session for a machine the shell has already looked at.
    pub fn with_context(backend: &'a (dyn SnapshotBackend + Sync), context: UiContext) -> Self {
        Self::start(State::with_context(context), Executor::new(backend))
    }

    /// A session on a machine that can answer questions about its disk, its jobs and its clock.
    pub fn with_system(
        backend: &'a (dyn SnapshotBackend + Sync),
        system: &'a (dyn SystemProbe + Sync),
        context: UiContext,
    ) -> Self {
        Self::start(
            State::with_context(context),
            Executor::new(backend).with_system(system),
        )
    }

    fn start(state: State, executor: Executor<'a>) -> Self {
        let mut session = Self { state, executor };
        session.dispatch(Msg::Start);
        session
    }

    pub fn press(&mut self, key: Key) {
        self.dispatch(Msg::Key(key));
    }

    /// Answer the question that is open, the way a person would: `y` for a change, `destroy` (and the name,
    /// where there is one) for what is lost for good. Nothing happens if no question is open.
    pub fn confirm(&mut self) {
        let Some(crate::model::Modal::Confirm(question)) = &self.state.modal else {
            return;
        };
        let answer = match (question.act.weight(), &question.target) {
            (snapall_core::Weight::Destroys, Some(name)) => format!("destroy {name}"),
            (snapall_core::Weight::Destroys, None) => "destroy".to_owned(),
            _ => "y".to_owned(),
        };
        for letter in answer.chars() {
            self.press(Key::Char(letter));
        }
        self.press(Key::Enter);
    }

    /// Process a message and every follow-up it causes, to completion.
    pub fn dispatch(&mut self, msg: Msg) {
        let mut queue = VecDeque::from([msg]);
        while let Some(msg) = queue.pop_front() {
            for effect in update(&mut self.state, msg) {
                queue.push_back(self.executor.execute(effect));
            }
        }
    }
}
