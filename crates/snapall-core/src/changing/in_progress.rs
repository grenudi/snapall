// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A change in progress: it holds the right to make a change (one at a time across every snapall, and a second is
//! told who has it and what for), and the journal knows of it from its start to its end, so that the window can show
//! what was done. Reading never needs it.

use crate::looking::journal::{Event, Outcome, Rollback, Source};
use crate::ports::machine::{Busy, Hold, SystemProbe};

/// The machine is not stored: each call is given it, so a change can be held by whoever owns the machine, or by
/// someone borrowing it.
#[must_use = "a change that is begun must be ended, so that the journal does not show it running for ever"]
pub struct InProgress {
    _hold: Hold,
    id: String,
    started: std::time::Instant,
}

impl InProgress {
    /// `now` is what is being done, for whoever is refused meanwhile; `done` is the same in the past tense, for the
    /// record. `pid` is the process making it, so that a change whose process is gone can be told from one that runs.
    pub fn begin(
        system: &dyn SystemProbe,
        now: &str,
        done: &str,
        source: Source,
        id: String,
        pid: u32,
    ) -> Result<InProgress, Busy> {
        let hold = system.hold_changes(now)?;
        system.record(&Event::Started {
            id: id.clone(),
            at: system.local_time().unwrap_or_default(),
            what: done.to_owned(),
            source,
            pid,
        });
        Ok(InProgress {
            _hold: hold,
            id,
            started: std::time::Instant::now(),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// How to get back from this change, worked out while the earlier state is known.
    pub fn rollback(&self, system: &dyn SystemProbe, rollback: Rollback) {
        system.record(&Event::Rollback {
            id: self.id.clone(),
            at: system.local_time().unwrap_or_default(),
            rollback,
        });
    }

    /// Step `done + 1` of `of` is about to begin, on `area`: say how far it has come, and whether to go on. `false` is a
    /// request to stop, and it stops the change between steps.
    pub fn proceed(&self, system: &dyn SystemProbe, done: usize, of: usize, area: &str) -> bool {
        system.record(&Event::Progress {
            id: self.id.clone(),
            at: system.local_time().unwrap_or_default(),
            done: u32::try_from(done).unwrap_or(u32::MAX),
            of: u32::try_from(of).unwrap_or(u32::MAX),
            area: area.to_owned(),
        });
        !system.stop_requested(&self.id)
    }

    /// How it went: what came of it, why it failed, or how far it got before it was stopped.
    pub fn end(self, system: &dyn SystemProbe, outcome: Outcome) {
        system.clear_stop(&self.id);
        system.record(&Event::Finished {
            id: self.id,
            at: system.local_time().unwrap_or_default(),
            outcome,
            took_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::testing::FakeSystem;

    fn begin(system: &FakeSystem) -> InProgress {
        InProgress::begin(
            system,
            "Taking snapshots",
            "Took snapshots",
            Source::Command,
            "42-1-c".into(),
            42,
        )
        .unwrap()
    }

    #[test]
    fn a_change_is_written_down_from_its_start_to_its_end() {
        let system = FakeSystem::new().at("2026-10-28 10:00:00");
        begin(&system).end(&system, Outcome::Done("two areas".into()));
        let events = system.events();
        assert!(
            matches!(&events[0], Event::Started { id, what, source: Source::Command, pid: 42, at } if id == "42-1-c" && what == "Took snapshots" && at == "2026-10-28 10:00:00")
        );
        assert!(
            matches!(&events[1], Event::Finished { id, outcome: Outcome::Done(what), .. } if id == "42-1-c" && what == "two areas")
        );
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn the_way_back_is_written_between_the_start_and_the_end() {
        let system = FakeSystem::new();
        let change = begin(&system);
        change.rollback(&system, Rollback::Advice("take a snapshot first".into()));
        change.end(&system, Outcome::Done(String::new()));
        let kinds: Vec<&str> = system
            .events()
            .iter()
            .map(|event| match event {
                Event::Started { .. } => "started",
                Event::Rollback { .. } => "rollback",
                Event::Finished { .. } => "finished",
                Event::Progress { .. } => "progress",
            })
            .collect();
        assert_eq!(kinds, ["started", "rollback", "finished"]);
    }

    #[test]
    fn a_second_change_is_refused_while_another_has_the_right_and_nothing_is_written() {
        let system = FakeSystem::new().busy_with("Restoring home");
        let refused = InProgress::begin(
            &system,
            "Deleting",
            "Deleted",
            Source::Window,
            "w".into(),
            7,
        );
        assert_eq!(
            refused.err().map(|busy| busy.doing),
            Some("Restoring home".to_owned())
        );
        assert!(
            system.events().is_empty(),
            "a refused change must leave no record of having started"
        );
    }

    #[test]
    fn each_step_is_reported_and_a_request_to_stop_is_seen_before_the_next() {
        let system = FakeSystem::new();
        let change = begin(&system);
        assert!(change.proceed(&system, 0, 3, "docs"));
        system.request_stop("42-1-c").unwrap();
        assert!(
            !change.proceed(&system, 1, 3, "home"),
            "asked to stop, it stops between steps"
        );
        let steps: Vec<(u32, &str)> = system
            .events()
            .iter()
            .filter_map(|event| match event {
                Event::Progress { done, area, .. } => Some((*done, area.as_str())),
                _ => None,
            })
            .map(|(done, area)| (done, if area == "docs" { "docs" } else { "home" }))
            .collect();
        assert_eq!(steps, [(0, "docs"), (1, "home")]);
        change.end(&system, Outcome::Cancelled("1 of 3".into()));
    }
}
