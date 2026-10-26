// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the machine can tell snapall besides what snapper says: how full a disk is, whether a
//! background job is switched on and running, what time it is there. A port, like the snapshot
//! backend: the core states what it needs, an adapter answers it, and nothing else knows how.

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
}

pub trait SystemProbe {
    /// The disk that holds `path`, or `None` if it cannot be read.
    fn disk_usage(&self, path: &str) -> Option<DiskUsage>;

    /// The state of a background job (a systemd unit such as `snapper-cleanup.timer`).
    fn service(&self, unit: &str) -> ServiceState;

    /// The local time as snapper writes dates (`2026-10-25 18:15:00`): the zone snapper's dates are
    /// in, so the two can be compared.
    fn local_time(&self) -> Option<String>;

    fn file_exists(&self, path: &str) -> bool;

    /// Switch a background job on and start it now (`systemctl enable --now`). It needs administrator
    /// rights, and says why in plain words when it cannot.
    fn enable_job(&self, unit: &str) -> Result<(), String>;
}

/// A machine that can be asked nothing: every answer is "unknown". What the window uses where no real
/// machine is given, so that it still shows what snapper reports.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoSystem;

impl SystemProbe for NoSystem {
    fn disk_usage(&self, _path: &str) -> Option<DiskUsage> {
        None
    }

    fn service(&self, _unit: &str) -> ServiceState {
        ServiceState::default()
    }

    fn local_time(&self) -> Option<String> {
        None
    }

    fn file_exists(&self, _path: &str) -> bool {
        false
    }

    fn enable_job(&self, _unit: &str) -> Result<(), String> {
        Err("this computer cannot be asked to start background jobs".to_owned())
    }
}
