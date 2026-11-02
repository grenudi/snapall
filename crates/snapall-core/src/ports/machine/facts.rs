// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the machine can report: how full a disk is, how a service stands, and what a unit says about itself.

/// How full the disk holding something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskUsage {
    pub size: u64,
    pub used: u64,
}

impl DiskUsage {
    /// Whole percent used, 0 to 100.
    pub fn percent_used(&self) -> u8 {
        if self.size == 0 {
            return 0;
        }
        u8::try_from((u128::from(self.used) * 100 / u128::from(self.size)).min(100)).unwrap_or(100)
    }
}

/// What the system says about one background job, as it said it: `enabled`, `disabled`, `active`,
/// `inactive`, `not-found`. `None` where it could not be asked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceState {
    pub enabled: Option<String>,
    pub active: Option<String>,
}

impl ServiceState {
    /// The job exists on this machine at all.
    pub fn is_installed(&self) -> bool {
        matches!(&self.enabled, Some(state) if state != "not-found")
    }

    pub fn is_running(&self) -> bool {
        self.active.as_deref() == Some("active")
    }

    /// How it stands, in a word or two for a person.
    pub fn words(&self) -> &'static str {
        match (
            self.is_installed(),
            self.is_running(),
            self.enabled.as_deref(),
        ) {
            (false, ..) => "not installed",
            (true, true, Some("disabled")) => "running, but not at start",
            (true, true, _) => "on",
            (true, false, Some("disabled")) => "off",
            (true, false, _) => "enabled but not running",
        }
    }
}

/// What a job's unit file says about it, as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitDetails {
    /// Where the unit file is.
    pub file: Option<String>,
    /// How the unit describes itself (`Description=`).
    pub description: Option<String>,
    /// Its timer settings (`OnBootSec=10m`) as key and value, in the order they apply.
    pub timer: Vec<(String, String)>,
}

impl UnitDetails {
    /// When it runs, in words.
    pub fn schedule(&self) -> Vec<String> {
        crate::keeping::jobs::schedule_words(&self.timer)
    }
}
