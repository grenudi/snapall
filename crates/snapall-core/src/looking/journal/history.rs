// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the events say, read back: each change as one entry, and whether it is still running.

use super::*;

/// Where a change stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Done,
    Failed,
    /// It was stopped on request, between steps, and says how far it had got.
    Cancelled,
    /// It started and its process is gone without having said how it ended.
    Interrupted,
}

impl Status {
    /// For a list: a word that needs no explaining.
    pub fn word(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Done => "done",
            Status::Failed => "FAILED",
            Status::Cancelled => "CANCELLED",
            Status::Interrupted => "STOPPED",
        }
    }
}

/// How far a change with several steps has come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub done: u32,
    pub of: u32,
    /// What the step now being taken is about.
    pub area: String,
}

/// A change, as the journal tells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The way back from it, if it was recorded.
    pub rollback: Option<Rollback>,
    /// How far it has come, while it runs and a step reported it.
    pub progress: Option<Progress>,
    pub id: String,
    pub what: String,
    pub source: Option<Source>,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub status: Status,
    /// What came of it, or why it failed; for a change that was interrupted, that it was.
    pub detail: String,
    pub took_ms: Option<u64>,
}

/// The events folded into one entry per change, newest first. `is_alive` says whether a process is still
/// running: a change that started and has not ended is running if its process is, and interrupted if not.
pub fn fold(events: &[Event], is_alive: impl Fn(u32) -> bool) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    for event in events {
        let at = entries.iter().position(|entry| entry.id == event.id());
        match (event, at) {
            (
                Event::Started {
                    id,
                    at,
                    what,
                    source,
                    pid,
                },
                None,
            ) => entries.push(Entry {
                id: id.clone(),
                what: what.clone(),
                source: Some(*source),
                pid: Some(*pid),
                rollback: None,
                progress: None,
                started_at: Some(at.clone()),
                finished_at: None,
                status: Status::Running,
                detail: String::new(),
                took_ms: None,
            }),
            // a repeated start of the same change changes nothing
            (Event::Started { .. }, Some(_)) => {}
            (Event::Progress { done, of, area, .. }, Some(index)) => {
                entries[index].progress = Some(Progress {
                    done: *done,
                    of: *of,
                    area: area.clone(),
                });
            }
            // a step of a change whose start is gone says nothing worth showing
            (Event::Progress { .. }, None) => {}
            (Event::Rollback { rollback, .. }, Some(index)) => {
                entries[index].rollback = Some(rollback.clone());
            }
            // a way back for a change whose start is gone belongs to nothing we show
            (Event::Rollback { .. }, None) => {}
            (
                Event::Finished {
                    id,
                    at,
                    outcome,
                    took_ms,
                },
                found,
            ) => {
                // the start may have been rotated away: the end alone is still worth showing
                let at_index = found.unwrap_or_else(|| {
                    entries.push(Entry {
                        id: id.clone(),
                        what: "An earlier change".to_owned(),
                        source: None,
                        pid: None,
                        rollback: None,
                        progress: None,
                        started_at: None,
                        finished_at: None,
                        status: Status::Running,
                        detail: String::new(),
                        took_ms: None,
                    });
                    entries.len() - 1
                });
                let entry = &mut entries[at_index];
                entry.finished_at = Some(at.clone());
                entry.took_ms = Some(*took_ms);
                match outcome {
                    Outcome::Done(done) => {
                        entry.status = Status::Done;
                        entry.detail.clone_from(done);
                    }
                    Outcome::Failed(why) => {
                        entry.status = Status::Failed;
                        entry.detail.clone_from(why);
                    }
                    Outcome::Cancelled(how_far) => {
                        entry.status = Status::Cancelled;
                        entry.detail.clone_from(how_far);
                    }
                }
                entry.progress = None;
            }
        }
    }
    for entry in &mut entries {
        if entry.status == Status::Running && !entry.pid.is_some_and(&is_alive) {
            entry.status = Status::Interrupted;
            entry.detail = "It was stopped before it could say how it ended: the program making it was closed or killed, or the computer was switched off. Look at the snapshots to see how far it got.".to_owned();
        }
    }
    entries.reverse();
    entries
}
