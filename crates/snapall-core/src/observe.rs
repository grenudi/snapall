// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The observability port: what snapall reports about what it did.
//!
//! The canon makes observability a first-class outbound port, not an afterthought. This module
//! is the *port*: pure data describing events and the trait that receives them. It has no clock
//! and no I/O. Measuring time, stamping it and writing a line live in the `snapall-observe`
//! adapter, which wraps any [`SnapshotBackend`](crate::SnapshotBackend), so the use cases
//! neither know nor change.

use std::time::Duration;

use crate::domain::{ConfigName, SnapshotId};

/// One thing snapall did, and how it went. Failures carry the error text, never a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ConfigsListed {
        result: Result<usize, String>,
        took: Duration,
    },
    SnapshotsListed {
        config: ConfigName,
        result: Result<usize, String>,
        took: Duration,
    },
    SnapshotCreated {
        config: ConfigName,
        description: String,
        result: Result<SnapshotId, String>,
        took: Duration,
    },
    SnapshotDeleted {
        config: ConfigName,
        id: SnapshotId,
        result: Result<(), String>,
        took: Duration,
    },
    /// A privileged command run by `snapall doctor --install`: worth an audit line.
    InstallStepRun {
        command: String,
        result: Result<(), String>,
        took: Duration,
    },
}

impl Event {
    /// A stable, machine-friendly name (`snapshot_created`), for log fields.
    pub fn name(&self) -> &'static str {
        match self {
            Event::ConfigsListed { .. } => "configs_listed",
            Event::SnapshotsListed { .. } => "snapshots_listed",
            Event::SnapshotCreated { .. } => "snapshot_created",
            Event::SnapshotDeleted { .. } => "snapshot_deleted",
            Event::InstallStepRun { .. } => "install_step_run",
        }
    }

    /// How long it took.
    pub fn took(&self) -> Duration {
        match self {
            Event::ConfigsListed { took, .. }
            | Event::SnapshotsListed { took, .. }
            | Event::SnapshotCreated { took, .. }
            | Event::SnapshotDeleted { took, .. }
            | Event::InstallStepRun { took, .. } => *took,
        }
    }

    /// The error text, if it failed.
    pub fn error(&self) -> Option<&str> {
        let error = match self {
            Event::ConfigsListed { result, .. } | Event::SnapshotsListed { result, .. } => {
                result.as_ref().err()
            }
            Event::SnapshotCreated { result, .. } => result.as_ref().err(),
            Event::SnapshotDeleted { result, .. } | Event::InstallStepRun { result, .. } => {
                result.as_ref().err()
            }
        };
        error.map(String::as_str)
    }

    pub fn is_failure(&self) -> bool {
        self.error().is_some()
    }

    /// The config this concerns, when there is one.
    pub fn config(&self) -> Option<&ConfigName> {
        match self {
            Event::SnapshotsListed { config, .. }
            | Event::SnapshotCreated { config, .. }
            | Event::SnapshotDeleted { config, .. } => Some(config),
            Event::ConfigsListed { .. } | Event::InstallStepRun { .. } => None,
        }
    }
}

/// Where events go.
///
/// Observing must never change what is being observed: an implementation that cannot deliver
/// an event (a full disk, a closed pipe) swallows the problem. Losing a log line is better than
/// failing a snapshot.
pub trait Observer {
    fn observe(&self, event: &Event);
}

impl<T: Observer + ?Sized> Observer for &T {
    fn observe(&self, event: &Event) {
        (**self).observe(event);
    }
}

impl<T: Observer + ?Sized> Observer for Box<T> {
    fn observe(&self, event: &Event) {
        (**self).observe(event);
    }
}

/// The observer that observes nothing: the default, and what tests use when they do not care.
#[derive(Debug, Clone, Copy, Default)]
pub struct Silent;

impl Observer for Silent {
    fn observe(&self, _event: &Event) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> ConfigName {
        ConfigName::parse(s).unwrap()
    }

    #[test]
    fn events_describe_themselves_for_a_log_line() {
        let ok = Event::SnapshotCreated {
            config: name("home"),
            description: "x".into(),
            result: Ok(SnapshotId::new(7).unwrap()),
            took: Duration::from_millis(12),
        };
        assert_eq!(ok.name(), "snapshot_created");
        assert_eq!(ok.took(), Duration::from_millis(12));
        assert_eq!(ok.config(), Some(&name("home")));
        assert!(!ok.is_failure());
        assert_eq!(ok.error(), None);

        let failed = Event::SnapshotDeleted {
            config: name("root"),
            id: SnapshotId::new(3).unwrap(),
            result: Err("not found".into()),
            took: Duration::ZERO,
        };
        assert!(failed.is_failure());
        assert_eq!(failed.error(), Some("not found"));
    }

    #[test]
    fn every_event_kind_has_a_distinct_name() {
        let d = Duration::ZERO;
        let names: Vec<&str> = [
            Event::ConfigsListed {
                result: Ok(0),
                took: d,
            },
            Event::SnapshotsListed {
                config: name("a"),
                result: Ok(0),
                took: d,
            },
            Event::SnapshotCreated {
                config: name("a"),
                description: String::new(),
                result: Err(String::new()),
                took: d,
            },
            Event::SnapshotDeleted {
                config: name("a"),
                id: SnapshotId::new(1).unwrap(),
                result: Ok(()),
                took: d,
            },
            Event::InstallStepRun {
                command: String::new(),
                result: Ok(()),
                took: d,
            },
        ]
        .iter()
        .map(Event::name)
        .collect();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "{names:?}");
    }

    #[test]
    fn an_observer_can_be_borrowed_or_boxed_without_changing_what_it_does() {
        use std::cell::Cell;
        struct Count(Cell<u32>);
        impl Observer for Count {
            fn observe(&self, _: &Event) {
                self.0.set(self.0.get() + 1);
            }
        }
        let count = Count(Cell::new(0));
        let event = Event::ConfigsListed {
            result: Ok(1),
            took: Duration::ZERO,
        };
        fn notify<O: Observer>(observer: O, event: &Event) {
            observer.observe(event);
        }
        notify(&count, &event); // through the `&T` impl
        notify(Box::new(Silent) as Box<dyn Observer>, &event); // through the `Box<T>` impl
        notify(&count, &event);
        assert_eq!(count.0.get(), 2);
    }
}
