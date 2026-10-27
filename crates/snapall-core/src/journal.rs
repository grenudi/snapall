// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall did, remembered: every change writes an event when it starts and another when it ends,
//! whichever snapall made it (the window or a command), and folding them gives the record a person reads.
//!
//! Two events instead of one, because a change that has started and never ends is itself a fact: it is
//! still running, or its process is gone (the computer was switched off, the terminal killed) and it was
//! *interrupted*, which a record written only at the end could never say.

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
    pub weight: crate::consequence::Weight,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn started(id: &str, what: &str, pid: u32) -> Event {
        Event::Started {
            id: id.into(),
            at: "2026-10-25 18:15:00".into(),
            what: what.into(),
            source: Source::Window,
            pid,
        }
    }

    fn finished(id: &str, outcome: Result<&str, &str>) -> Event {
        Event::Finished {
            id: id.into(),
            at: "2026-10-25 18:15:03".into(),
            outcome: outcome.map(str::to_owned).map_err(str::to_owned).into(),
            took_ms: 3000,
        }
    }

    #[test]
    fn a_change_that_started_and_ended_is_done_or_failed_with_what_came_of_it() {
        let entries = fold(
            &[
                started("a", "Took a snapshot of home", 10),
                finished("a", Ok("1 snapshot taken")),
                started("b", "Deleted 1 snapshot of home", 10),
                finished("b", Err("snapshot #3 does not exist")),
            ],
            |_| true,
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(
            (entries[0].id.as_str(), entries[0].status),
            ("b", Status::Failed),
            "newest first"
        );
        assert_eq!(entries[0].detail, "snapshot #3 does not exist");
        assert_eq!(
            (entries[1].status, entries[1].detail.as_str()),
            (Status::Done, "1 snapshot taken")
        );
        assert_eq!(entries[1].took_ms, Some(3000));
        assert_eq!(
            entries[1].finished_at.as_deref(),
            Some("2026-10-25 18:15:03")
        );
    }

    #[test]
    fn a_change_that_started_and_never_ended_is_running_if_its_process_is_and_interrupted_if_not() {
        let events = [started("a", "Putting Home back", 4242)];
        assert_eq!(fold(&events, |pid| pid == 4242)[0].status, Status::Running);
        let gone = fold(&events, |_| false);
        assert_eq!(gone[0].status, Status::Interrupted);
        assert!(
            gone[0]
                .detail
                .contains("stopped before it could say how it ended"),
            "{}",
            gone[0].detail
        );
        assert!(
            gone[0].detail.contains("Look at the snapshots"),
            "it says what to do next"
        );
    }

    #[test]
    fn a_change_that_did_end_is_never_called_interrupted_whatever_its_process() {
        let events = [started("a", "x", 1), finished("a", Ok(""))];
        assert_eq!(fold(&events, |_| false)[0].status, Status::Done);
    }

    #[test]
    fn an_end_whose_start_was_rotated_away_is_still_shown_and_a_repeated_start_changes_nothing() {
        let alone = fold(&[finished("old", Ok("done"))], |_| false);
        assert_eq!(
            (alone[0].what.as_str(), alone[0].status),
            ("An earlier change", Status::Done)
        );
        let twice = fold(
            &[
                started("a", "x", 1),
                started("a", "x", 1),
                finished("a", Ok("")),
            ],
            |_| true,
        );
        assert_eq!(twice.len(), 1);
    }

    #[test]
    fn interleaved_changes_of_different_programs_are_kept_apart_by_their_ids() {
        let entries = fold(
            &[
                started("w", "Window change", 1),
                started("c", "Command change", 2),
                finished("c", Ok("done")),
                finished("w", Err("boom")),
            ],
            |_| true,
        );
        let by = |id: &str| entries.iter().find(|e| e.id == id).unwrap().status;
        assert_eq!((by("w"), by("c")), (Status::Failed, Status::Done));
    }

    #[test]
    fn the_words_for_a_source_round_trip() {
        for source in [Source::Window, Source::Command] {
            assert_eq!(Source::from_word(source.word()), Some(source));
        }
        assert_eq!(Source::from_word("other"), None);
    }

    #[test]
    fn a_way_back_recorded_for_a_change_is_part_of_its_entry_whenever_it_was_recorded() {
        let undo = Rollback::Undo(Undo {
            what: "Put Home back to how it was: snapshot #12".into(),
            commands: vec![vec!["restore".into(), "home".into(), "12".into()]],
            weight: crate::consequence::Weight::Changes,
        });
        let way_back = |rollback: Rollback| Event::Rollback {
            id: "a".into(),
            at: "t".into(),
            rollback,
        };
        // before the change ended, and after it
        for events in [
            vec![
                started("a", "x", 1),
                way_back(undo.clone()),
                finished("a", Ok("")),
            ],
            vec![
                started("a", "x", 1),
                finished("a", Ok("")),
                way_back(undo.clone()),
            ],
        ] {
            assert_eq!(fold(&events, |_| true)[0].rollback, Some(undo.clone()));
        }
        let advice = Rollback::Advice("Deleted snapshots are gone.".into());
        let entries = fold(&[started("b", "y", 1), way_back(advice.clone())], |_| true);
        assert_eq!(
            entries[0].rollback, None,
            "advice for another change is not this one's"
        );
        assert!(
            fold(&[way_back(advice)], |_| true).is_empty(),
            "a way back alone is not a change"
        );
    }

    #[test]
    fn a_way_back_is_worded_once_for_whoever_shows_it() {
        let undo = Rollback::Undo(Undo {
            what: "Put Home back to how it was".into(),
            commands: vec![],
            weight: crate::consequence::Weight::Changes,
        });
        assert_eq!(
            undo.words("42-1-c"),
            [
                "Way back: Put Home back to how it was.",
                "To do it: snapall undo 42-1-c"
            ]
        );
        let advice = Rollback::Advice("Deleted snapshots are gone.".into());
        assert_eq!(
            advice.words("42-1-c"),
            ["It cannot be undone. Deleted snapshots are gone."]
        );
    }
}
