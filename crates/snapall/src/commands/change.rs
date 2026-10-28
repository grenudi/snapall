// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A change being made: it is started and written down, followed while it runs (can it be asked to stop?), and ended with how it went.

use std::error::Error;

/// A change in progress: it holds the right to make a change (only one snapall does at a time, and a
/// second is told who has it and what for), and the journal knows of it from its start to its end, so
/// that the window can show what a command did. Reading never needs it.
pub(super) struct Change {
    _hold: snapall_core::Hold,
    id: String,
    system: snapall_system::LinuxSystem,
    started: std::time::Instant,
    ended: bool,
}

impl Change {
    /// `now` is what is being done, for whoever is refused meanwhile; `done` is the same in the past
    /// tense, for the record.
    /// A change begins only with leave to make it.
    pub(super) fn begin(
        _approved: crate::ask::Approved,
        now: &str,
        done: &str,
        name: Option<&str>,
    ) -> Result<Change, Box<dyn Error>> {
        Change::start(now, done, name)
    }

    /// The one change that asks as it goes: `setup` asks area by area while it works, so there is no single leave to
    /// show first. A test keeps it to that one place.
    pub(super) fn begin_asking_along(
        now: &str,
        done: &str,
        name: Option<&str>,
    ) -> Result<Change, Box<dyn Error>> {
        Change::start(now, done, name)
    }

    fn start(now: &str, done: &str, name: Option<&str>) -> Result<Change, Box<dyn Error>> {
        use snapall_core::SystemProbe;
        let system = snapall_system::LinuxSystem::new();
        let hold = system.hold_changes(now).map_err(|busy| busy.to_string())?;
        let milliseconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis());
        let id = name.map_or_else(
            || format!("{}-{milliseconds}-c", std::process::id()),
            str::to_owned,
        );
        // a change that the window handed to the background says so; any other is a command
        let source = match std::env::var("SNAPALL_SOURCE").as_deref() {
            Ok("window") => snapall_core::Source::Window,
            _ => snapall_core::Source::Command,
        };
        system.record(&snapall_core::JournalEvent::Started {
            id: id.clone(),
            at: system.local_time().unwrap_or_default(),
            what: done.to_owned(),
            source,
            pid: std::process::id(),
        });
        Ok(Change {
            _hold: hold,
            id,
            system,
            started: std::time::Instant::now(),
            ended: false,
        })
    }

    /// How to get back from this change, worked out now, while the earlier state is known.
    pub(super) fn rollback(&self, rollback: snapall_core::Rollback) {
        use snapall_core::SystemProbe;
        self.system.record(&snapall_core::JournalEvent::Rollback {
            id: self.id.clone(),
            at: self.system.local_time().unwrap_or_default(),
            rollback,
        });
    }

    /// How it went: what came of it, or why it failed.
    pub(super) fn end_with<T, E: ToString>(self, result: &Result<T, E>) {
        self.end(
            result
                .as_ref()
                .map(|_| String::new())
                .map_err(ToString::to_string),
        );
    }

    pub(super) fn end(mut self, outcome: Result<String, String>) {
        self.write_end(outcome.into());
    }

    /// It was stopped on request, between steps; how far it had got.
    pub(super) fn end_stopped(mut self, how_far: String) {
        self.write_end(snapall_core::JournalOutcome::Cancelled(how_far));
    }

    /// The steps it takes are reported as they begin, and a request to stop is looked for before each.
    pub(super) fn course(&self) -> Following<'_> {
        Following { change: self }
    }

    pub(super) fn write_end(&mut self, outcome: snapall_core::JournalOutcome) {
        use snapall_core::SystemProbe;
        self.ended = true;
        self.system.clear_stop(&self.id);
        self.system.record(&snapall_core::JournalEvent::Finished {
            id: self.id.clone(),
            at: self.system.local_time().unwrap_or_default(),
            outcome,
            took_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
        });
    }
}

impl Drop for Change {
    /// A command that leaves early, on an error, did not finish: the record says so rather than leaving
    /// a change that looks as if it were still running.
    fn drop(&mut self) {
        if !self.ended {
            self.write_end(snapall_core::JournalOutcome::Failed(
                "it ended early, with an error that the command printed".to_owned(),
            ));
        }
    }
}

/// What a change does between its steps: say how far it has come, and look for a request to stop.
pub(super) struct Following<'a> {
    change: &'a Change,
}

impl snapall_core::Course for Following<'_> {
    fn proceed(&self, done: usize, of: usize, area: &str) -> bool {
        use snapall_core::SystemProbe;
        let system = &self.change.system;
        system.record(&snapall_core::JournalEvent::Progress {
            id: self.change.id.clone(),
            at: system.local_time().unwrap_or_default(),
            done: u32::try_from(done).unwrap_or(u32::MAX),
            of: u32::try_from(of).unwrap_or(u32::MAX),
            area: area.to_owned(),
        });
        !system.stop_requested(&self.change.id)
    }
}
