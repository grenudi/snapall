// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A change being made from the command line: it is started and written down, followed while it runs (can it be asked to
//! stop?), and ended with how it went.

use std::error::Error;

use snapall_core::{InProgress, JournalOutcome};

/// A change in progress, held by the command that makes it. What a change is, and how it is written down, is the
/// core's ([`InProgress`]); this is the command line's side of it: where it came from, what it is called, and that
/// a command that leaves early did not finish, so the record says so.
pub(super) struct Change {
    system: snapall_system::LinuxSystem,
    progress: Option<InProgress>,
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
        let system = snapall_system::LinuxSystem::new();
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
        let progress = InProgress::begin(&system, now, done, source, id, std::process::id())
            .map_err(|busy| busy.to_string())?;
        Ok(Change {
            system,
            progress: Some(progress),
        })
    }

    /// How to get back from this change, worked out now, while the earlier state is known.
    pub(super) fn rollback(&self, rollback: snapall_core::Rollback) {
        if let Some(progress) = &self.progress {
            progress.rollback(&self.system, rollback);
        }
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
        self.finish(outcome.into());
    }

    /// It was stopped on request, between steps; how far it had got.
    pub(super) fn end_stopped(mut self, how_far: String) {
        self.finish(JournalOutcome::Cancelled(how_far));
    }

    /// The steps it takes are reported as they begin, and a request to stop is looked for before each.
    pub(super) fn course(&self) -> Following<'_> {
        Following { change: self }
    }

    fn finish(&mut self, outcome: JournalOutcome) {
        if let Some(progress) = self.progress.take() {
            progress.end(&self.system, outcome);
        }
    }
}

impl Drop for Change {
    /// A command that leaves early, on an error, did not finish: the record says so rather than leaving
    /// a change that looks as if it were still running.
    fn drop(&mut self) {
        self.finish(JournalOutcome::Failed(
            "it ended early, with an error that the command printed".to_owned(),
        ));
    }
}

/// What a change does between its steps: say how far it has come, and look for a request to stop.
pub(super) struct Following<'a> {
    change: &'a Change,
}

impl snapall_core::Course for Following<'_> {
    fn proceed(&self, done: usize, of: usize, area: &str) -> bool {
        match &self.change.progress {
            Some(progress) => progress.proceed(&self.change.system, done, of, area),
            None => true,
        }
    }
}
