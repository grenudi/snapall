// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Collecting what is true of the machine: each area's snapshots and limits, the jobs, the room taken.

use crate::keeping::jobs::{self, Job};
use crate::ports::machine::{DiskUsage, QuotaMode, ServiceState, SystemProbe, UnitDetails};
use crate::ports::snapshots::{BackendError, SnapshotBackend};
use crate::protecting::coverage::describe;
use crate::vocabulary::domain::{AreaName, Retention, SnapshotInfo};
use crate::vocabulary::explain::Limit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaFacts {
    pub config: AreaName,
    /// The directory it protects: `/`, `/home`.
    pub location: String,
    pub snapshots: usize,
    /// When the oldest and the newest were taken, as snapper wrote it.
    pub oldest: Option<String>,
    pub newest: Option<String>,
    /// The bytes the snapshots hold, when every one of them was measured; `None` if any was not.
    pub measured_bytes: Option<u64>,
    pub retention: Option<Retention>,
    pub disk: Option<DiskUsage>,
    /// How the disk holding it counts room.
    pub quotas: QuotaMode,
}

impl AreaFacts {
    /// What a person calls it: `Home`.
    pub fn title(&self) -> String {
        describe(&self.location).title
    }

    /// Where its snapshots are kept.
    pub fn lives_in(&self) -> String {
        format!("{}/.snapshots", self.location.trim_end_matches('/'))
    }
}

/// A background job, as the machine reports it and as its unit file describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobFacts {
    pub job: Job,
    pub state: ServiceState,
    pub details: Option<UnitDetails>,
}

/// Everything the report is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The local time, as snapper writes dates, if it could be read.
    pub now: Option<String>,
    pub areas: Vec<AreaFacts>,
    /// Every job in the catalogue (`jobs::ALL`).
    pub jobs: Vec<JobFacts>,
    /// The boot menu is kept in step by a snapper plugin instead of a background job.
    pub boot_menu_plugin: bool,
}

impl Facts {
    pub fn job(&self, job: &Job) -> Option<&JobFacts> {
        self.jobs.iter().find(|facts| facts.job.unit == job.unit)
    }

    /// How the job stands; a job that is not here is not installed.
    pub fn state_of(&self, job: &Job) -> ServiceState {
        self.job(job)
            .map(|facts| facts.state.clone())
            .unwrap_or_default()
    }

    /// When the job runs, in words, as its own unit file says.
    pub fn schedule_of(&self, job: &Job) -> Vec<String> {
        self.job(job)
            .and_then(|facts| facts.details.as_ref())
            .map(UnitDetails::schedule)
            .unwrap_or_default()
    }

    /// What each area keeps, and protects from cleanup: its real settings.
    pub fn cleanup_limits(&self) -> Vec<Limit> {
        self.areas
            .iter()
            .filter_map(|area| {
                let retention = area.retention.as_ref()?;
                Some(Limit {
                    area: area.title(),
                    keep: retention.keep,
                    min_age_seconds: retention.min_age_seconds,
                })
            })
            .collect()
    }
}

/// Read everything the report is made from. An area whose snapshots cannot be listed is left out
/// rather than failing the whole report: the rest is still worth showing.
pub fn gather(
    backend: &dyn SnapshotBackend,
    system: &dyn SystemProbe,
) -> Result<Facts, BackendError> {
    let areas = backend
        .list_areas()?
        .into_iter()
        .filter_map(|config| area_facts(backend, system, config))
        .collect();
    Ok(Facts {
        now: system.local_time(),
        areas,
        jobs: jobs::ALL
            .iter()
            .map(|job| JobFacts {
                job: *job,
                state: system.service(job.unit),
                details: system.unit_details(job.unit),
            })
            .collect(),
        boot_menu_plugin: system.file_exists("/usr/lib/snapper/plugins/10-limine-snapper-sync"),
    })
}

pub(super) fn area_facts(
    backend: &dyn SnapshotBackend,
    system: &dyn SystemProbe,
    config: AreaName,
) -> Option<AreaFacts> {
    let location = backend.area_location(&config).ok()?;
    let snapshots = backend.list_snapshots(&config).ok()?;
    Some(AreaFacts {
        retention: backend.retention(&config).ok(),
        disk: system.disk_usage(&location),
        quotas: system.quota_mode(&location),
        oldest: snapshots.iter().filter_map(|s| s.date.clone()).min(),
        newest: snapshots.iter().filter_map(|s| s.date.clone()).max(),
        measured_bytes: measured_bytes(&snapshots),
        snapshots: snapshots.len(),
        location,
        config,
    })
}

/// The bytes the snapshots hold, if every one of them was measured.
pub(super) fn measured_bytes(snapshots: &[SnapshotInfo]) -> Option<u64> {
    snapshots.iter().map(|s| s.used_bytes).sum()
}
