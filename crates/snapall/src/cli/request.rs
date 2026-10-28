// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What was asked for: the request, how it logs, and the error in asking.

use crate::contract::LogFormat;
use argenv_resolve::generate::Shell;
use snapall_core::{AreaName, Description, Point, SnapshotId};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Help,
    Version,
    Create {
        only: Option<Vec<AreaName>>,
        description: Description,
        dry_run: bool,
        job: Option<String>,
        group: Option<String>,
    },
    List,
    Snapshots {
        config: Option<AreaName>,
    },
    Delete {
        config: AreaName,
        id: SnapshotId,
        destroy: bool,
    },
    /// What restoring snapshot `id` would change.
    Changes {
        config: AreaName,
        id: SnapshotId,
        path: Option<String>,
    },
    /// Put the config, or one path of it, back to how snapshot `id` had it.
    Restore {
        config: AreaName,
        id: SnapshotId,
        path: Option<String>,
        dry_run: bool,
        yes: bool,
    },
    /// Every restore point.
    Points,
    /// How protection stands: room used, cleanup, services.
    Health,
    /// What snapall has done, newest first.
    Activity,
    /// Sets of areas that have a name.
    Groups {
        action: Option<String>,
        name: Option<String>,
        areas: Option<String>,
        yes: bool,
    },
    /// How many snapshots an area keeps: show it, or change it.
    Limits {
        area: String,
        keep: Option<String>,
        min_age: Option<String>,
        cleanup: Option<String>,
        yes: bool,
        destroy: bool,
    },
    /// Stop protecting an area (deletes its snapshots).
    Unprotect {
        area: String,
        dry_run: bool,
        destroy: bool,
    },
    /// Roll a change back, if a way back was recorded.
    Undo {
        change: String,
        yes: bool,
        destroy: bool,
    },
    /// Ask a running change to stop after the area it is on.
    Cancel {
        job: String,
        yes: bool,
    },
    /// How the disk counts the room snapshots take; with `on` or `off`, switch it.
    Quotas {
        state: Option<String>,
        yes: bool,
    },
    /// Put every area back to one restore point.
    RestoreAll {
        point: Point,
        yes: bool,
        dry_run: bool,
        job: Option<String>,
    },
    /// How one file stood in each snapshot.
    History {
        path: String,
    },
    /// Protect the parts of the disk that snapshots do not cover yet.
    Setup {
        area: Option<String>,
        yes: bool,
        dry_run: bool,
    },
    Doctor {
        install: bool,
        yes: bool,
    },
    Completions {
        shell: Shell,
    },
    Man {
        /// The man page date as seconds since the epoch; `None` means now.
        source_date_epoch: Option<u64>,
    },
    Tui {
        /// Open on the history of this file instead of on all configs.
        file: Option<String>,
    },
    Contract,
}

/// Where and how events are written. Resolved from `--log`, `--log-file` and their variables.
#[derive(Debug, PartialEq, Eq)]
pub struct Logging {
    pub format: LogFormat,
    pub file: Option<PathBuf>,
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            format: LogFormat::Off,
            file: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub request: Request,
    pub no_dbus: bool,
    pub json: bool,
    /// Whether tips (such as "snapshots can be booted") may be shown.
    pub hints: bool,
    pub logging: Logging,
    /// Problems worth showing that do not stop the run (for example a misspelled
    /// `SNAPALL_*` variable).
    pub warnings: Vec<String>,
}

/// The invocation is wrong. Reported with exit status 2.
#[derive(Debug, PartialEq, Eq)]
pub struct UsageError(pub Vec<String>);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("\n"))
    }
}

impl std::error::Error for UsageError {}

pub(super) fn usage_error(message: impl Into<String>) -> UsageError {
    UsageError(vec![message.into()])
}
