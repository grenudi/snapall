// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What was written when a change began, went on and ended: the events, and how a change can be undone.

/// Who made the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The interactive window.
    Window,
    /// A command typed or scripted.
    Command,
}

impl Source {
    pub fn word(self) -> &'static str {
        match self {
            Source::Window => "window",
            Source::Command => "command",
        }
    }

    pub fn from_word(word: &str) -> Option<Source> {
        match word {
            "window" => Some(Source::Window),
            "command" => Some(Source::Command),
            _ => None,
        }
    }
}

/// How a change ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It went through; what came of it.
    Done(String),
    /// It did not; why.
    Failed(String),
    /// It was stopped on request, between steps; how far it had got.
    Cancelled(String),
}

impl Outcome {
    /// How it ended, in the word the journal stores.
    pub fn word(&self) -> &'static str {
        match self {
            Outcome::Done(_) => "done",
            Outcome::Failed(_) => "failed",
            Outcome::Cancelled(_) => "cancelled",
        }
    }

    /// What came of it, or why it did not, or how far it had got.
    pub fn detail(&self) -> &str {
        match self {
            Outcome::Done(text) | Outcome::Failed(text) | Outcome::Cancelled(text) => text,
        }
    }

    /// The outcome a stored word and detail say, or `None` for a word this does not know.
    pub fn from_word(word: &str, detail: String) -> Option<Outcome> {
        match word {
            "done" => Some(Outcome::Done(detail)),
            "failed" => Some(Outcome::Failed(detail)),
            "cancelled" => Some(Outcome::Cancelled(detail)),
            _ => None,
        }
    }
}

impl From<Result<String, String>> for Outcome {
    fn from(result: Result<String, String>) -> Self {
        match result {
            Ok(done) => Outcome::Done(done),
            Err(why) => Outcome::Failed(why),
        }
    }
}

/// The way back from a change, worked out when it was made, so that it knows the state that was actually
/// there, not a guess made later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undo {
    /// What it would put back, in a sentence that names the earlier state: `Put Home back to how it was at
    /// 18:15: snapshot #12`.
    pub what: String,
    /// The snapall commands that do it, one after the other.
    pub commands: Vec<Vec<String>>,
    /// What doing it weighs: undoing the taking of a snapshot is deleting it.
    pub weight: crate::vocabulary::consequence::Weight,
}

/// What can be done about a change afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rollback {
    /// It can be undone, and this says how.
    Undo(Undo),
    /// It cannot be undone, and this says what protects against it the next time.
    Advice(String),
}

impl Rollback {
    /// What to tell a person about it, for the change called `id`: the state it returns to and how to do it,
    /// or that it cannot be undone and what protects them next time. Two lines at most, no indentation: the
    /// caller lays them out. The command line and the window both say it this way.
    pub fn words(&self, id: &str) -> Vec<String> {
        match self {
            Rollback::Undo(undo) => vec![
                format!("Way back: {}.", undo.what),
                format!("To do it: snapall undo {id}"),
            ],
            Rollback::Advice(text) => vec![format!("It cannot be undone. {text}")],
        }
    }
}

/// One line of the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Started {
        id: String,
        /// When, as the machine's clock wrote it (`2026-10-25 18:15:02`).
        at: String,
        /// What it is, in a sentence: `Took a snapshot of home, root`.
        what: String,
        source: Source,
        /// The process making it, so that a change whose process is gone can be told from one that runs.
        pid: u32,
    },
    /// A step of a change that has several is about to begin: how far it has come.
    Progress {
        id: String,
        at: String,
        /// Steps finished so far.
        done: u32,
        /// Steps in all.
        of: u32,
        /// What the step that begins now is about.
        area: String,
    },
    Finished {
        id: String,
        at: String,
        outcome: Outcome,
        took_ms: u64,
    },
    /// The way back from the change, once it is known.
    Rollback {
        id: String,
        at: String,
        rollback: Rollback,
    },
}

impl Event {
    pub fn id(&self) -> &str {
        match self {
            Event::Started { id, .. }
            | Event::Progress { id, .. }
            | Event::Finished { id, .. }
            | Event::Rollback { id, .. } => id,
        }
    }
}
