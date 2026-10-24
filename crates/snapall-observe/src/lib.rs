// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Adapter for the observability port.
//!
//! [`Observed`] wraps any [`SnapshotBackend`], times every call and reports it to an
//! [`Observer`]; [`Lines`] is an observer that writes one line per event, as text or as JSON.
//! Together they give snapall a structured event stream (twelve-factor: logs go to a stream
//! the environment routes) and an audit trail of everything it did to the system, without a
//! single use case knowing about either.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod clock;
mod lines;

use std::time::Instant;

use snapall_core::{
    BackendError, Change, ConfigName, Description, Event, FileVersion, Observer, RestoreSummary,
    SnapshotBackend, SnapshotId, SnapshotInfo,
};

pub use clock::rfc3339_utc;
pub use lines::{Format, Lines};

/// A backend that reports what it does. It returns exactly what the wrapped backend returns, so
/// it passes the same contract (and a test checks that).
pub struct Observed<B, O> {
    inner: B,
    observer: O,
}

impl<B, O> Observed<B, O> {
    pub fn new(inner: B, observer: O) -> Self {
        Self { inner, observer }
    }
}

#[cfg(test)]
impl<O> Observed<snapall_core::testing::InMemoryBackend, O> {
    /// Edit a live file of the wrapped in-memory backend (tests only).
    fn write_file_for_test(&self, path: &str, content: &str) {
        self.inner.write_file("home", path, content);
    }
}

fn message<T>(result: &Result<T, BackendError>) -> Result<(), String> {
    result.as_ref().map(|_| ()).map_err(ToString::to_string)
}

impl<B: SnapshotBackend, O: Observer> SnapshotBackend for Observed<B, O> {
    fn list_configs(&self) -> Result<Vec<ConfigName>, BackendError> {
        let start = Instant::now();
        let result = self.inner.list_configs();
        self.observer.observe(&Event::ConfigsListed {
            result: result.as_ref().map(Vec::len).map_err(ToString::to_string),
            took: start.elapsed(),
        });
        result
    }

    fn create_snapshot(
        &self,
        config: &ConfigName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError> {
        let start = Instant::now();
        let result = self.inner.create_snapshot(config, description);
        self.observer.observe(&Event::SnapshotCreated {
            config: config.clone(),
            description: description.as_str().to_owned(),
            result: result.as_ref().map(|id| *id).map_err(ToString::to_string),
            took: start.elapsed(),
        });
        result
    }

    fn list_snapshots(&self, config: &ConfigName) -> Result<Vec<SnapshotInfo>, BackendError> {
        let start = Instant::now();
        let result = self.inner.list_snapshots(config);
        self.observer.observe(&Event::SnapshotsListed {
            config: config.clone(),
            result: result.as_ref().map(Vec::len).map_err(ToString::to_string),
            took: start.elapsed(),
        });
        result
    }

    fn delete_snapshot(&self, config: &ConfigName, id: SnapshotId) -> Result<(), BackendError> {
        let start = Instant::now();
        let result = self.inner.delete_snapshot(config, id);
        self.observer.observe(&Event::SnapshotDeleted {
            config: config.clone(),
            id,
            result: message(&result),
            took: start.elapsed(),
        });
        result
    }

    // Reading where a config lives and what changed is not an event; only the mutation is.
    fn config_subvolume(&self, config: &ConfigName) -> Result<String, BackendError> {
        self.inner.config_subvolume(config)
    }

    fn changes_since(
        &self,
        config: &ConfigName,
        id: SnapshotId,
    ) -> Result<Vec<Change>, BackendError> {
        self.inner.changes_since(config, id)
    }

    fn file_versions(
        &self,
        config: &ConfigName,
        path: &str,
    ) -> Result<Vec<FileVersion>, BackendError> {
        self.inner.file_versions(config, path)
    }

    fn undo_changes(
        &self,
        config: &ConfigName,
        id: SnapshotId,
        path: Option<&str>,
    ) -> Result<RestoreSummary, BackendError> {
        let start = Instant::now();
        let result = self.inner.undo_changes(config, id, path);
        self.observer.observe(&Event::ChangesUndone {
            config: config.clone(),
            id,
            path: path.map(str::to_owned),
            result: result
                .as_ref()
                .map(|summary| *summary)
                .map_err(ToString::to_string),
            took: start.elapsed(),
        });
        result
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use snapall_core::testing::{backend_contract, InMemoryBackend};

    use super::*;

    /// Remembers every event.
    #[derive(Default)]
    struct Recorder(RefCell<Vec<Event>>);

    impl Observer for Recorder {
        fn observe(&self, event: &Event) {
            self.0.borrow_mut().push(event.clone());
        }
    }

    fn name(s: &str) -> ConfigName {
        ConfigName::parse(s).unwrap()
    }

    #[test]
    fn an_observed_backend_still_satisfies_the_backend_contract() {
        let recorder = Recorder::default();
        let backend = Observed::new(InMemoryBackend::with_configs(&["root"]), &recorder);
        backend_contract(&backend, &name("root"));
        assert!(
            !recorder.0.borrow().is_empty(),
            "the contract's calls were observed"
        );
    }

    #[test]
    fn every_operation_is_reported_with_its_outcome() {
        let recorder = Recorder::default();
        let backend = Observed::new(
            InMemoryBackend::with_configs(&["home", "root"]).failing_on("root"),
            &recorder,
        );
        let description = Description::parse("before theming").unwrap();

        backend.list_configs().unwrap();
        let id = backend
            .create_snapshot(&name("home"), &description)
            .unwrap();
        backend
            .create_snapshot(&name("root"), &description)
            .unwrap_err();
        backend.list_snapshots(&name("home")).unwrap();
        backend.delete_snapshot(&name("home"), id).unwrap();
        backend.delete_snapshot(&name("home"), id).unwrap_err();

        let events = recorder.0.borrow();
        let names: Vec<&str> = events.iter().map(Event::name).collect();
        assert_eq!(
            names,
            [
                "configs_listed",
                "snapshot_created",
                "snapshot_created",
                "snapshots_listed",
                "snapshot_deleted",
                "snapshot_deleted"
            ]
        );
        assert!(matches!(
            &events[0],
            Event::ConfigsListed { result: Ok(2), .. }
        ));
        assert!(matches!(
            &events[1],
            Event::SnapshotCreated { description, result: Ok(_), .. } if description == "before theming"
        ));
        assert!(
            events[2].is_failure(),
            "the injected failure is reported, with its text"
        );
        assert!(events[2].error().unwrap().contains("refused"));
        assert!(matches!(
            &events[3],
            Event::SnapshotsListed { result: Ok(1), .. }
        ));
        assert!(!events[4].is_failure());
        assert!(
            events[5].is_failure(),
            "deleting twice is reported as a failure"
        );
    }

    #[test]
    fn what_the_wrapped_backend_returns_is_returned_unchanged() {
        let plain = InMemoryBackend::with_configs(&["home"]);
        let observed = Observed::new(
            InMemoryBackend::with_configs(&["home"]),
            Recorder::default(),
        );
        let description = Description::default();
        assert_eq!(
            plain.create_snapshot(&name("home"), &description).unwrap(),
            observed
                .create_snapshot(&name("home"), &description)
                .unwrap()
        );
        assert_eq!(
            plain.list_snapshots(&name("home")).unwrap(),
            observed.list_snapshots(&name("home")).unwrap()
        );
    }

    #[test]
    fn a_restore_is_an_audited_event_with_what_it_did_and_reading_is_not() {
        let recorder = Recorder::default();
        let inner = InMemoryBackend::with_configs(&["home"]);
        inner.write_file("home", "/mnt/home/a.txt", "one");
        let backend = Observed::new(inner, &recorder);
        let id = backend
            .create_snapshot(&name("home"), &Description::default())
            .unwrap();
        backend.write_file_for_test("/mnt/home/a.txt", "two");

        backend.config_subvolume(&name("home")).unwrap();
        backend.changes_since(&name("home"), id).unwrap();
        let before_undo = recorder.0.borrow().len();
        assert_eq!(
            before_undo, 1,
            "only the creation so far: reads are not events"
        );

        let summary = backend
            .undo_changes(&name("home"), id, Some("/mnt/home/a.txt"))
            .unwrap();
        assert_eq!(summary.modified, 1);
        let events = recorder.0.borrow();
        assert!(matches!(
            events.last().unwrap(),
            Event::ChangesUndone { path: Some(p), result: Ok(s), .. } if p == "/mnt/home/a.txt" && s.modified == 1
        ));

        // A failed restore is audited as a failure, with the reason.
        drop(events);
        let nowhere = SnapshotId::new(99).unwrap();
        backend
            .undo_changes(&name("home"), nowhere, None)
            .unwrap_err();
        assert!(recorder.0.borrow().last().unwrap().is_failure());
    }
}
