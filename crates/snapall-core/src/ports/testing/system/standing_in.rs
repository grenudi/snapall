// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A stand-in for the machine: how a test arranges what it says, and how it looks at what it was told.

use crate::keeping::groups::Groups;
use crate::keeping::jobs::JobAction;
use crate::looking::journal;
use crate::ports::machine::{Busy, DiskUsage, QuotaMode, ServiceState, UnitDetails};
use std::collections::BTreeMap;

/// A machine whose answers are set by the test: how full its disk is, which jobs run, what time it is,
/// how room is counted, who else is making a change.
#[derive(Default)]
pub struct FakeSystem {
    pub(super) disk: Option<DiskUsage>,
    pub(super) services: std::sync::Mutex<BTreeMap<String, ServiceState>>,
    pub(super) details: BTreeMap<String, UnitDetails>,
    pub(super) time: Option<String>,
    pub(super) boot_menu_plugin: bool,
    pub(super) refuse_jobs: bool,
    pub(super) actions: std::sync::Mutex<Vec<(String, JobAction)>>,
    pub(super) quotas: std::sync::Mutex<Option<QuotaMode>>,
    pub(super) busy: Option<Busy>,
    pub(super) journal: std::sync::Mutex<Vec<journal::Event>>,
    pub(super) background: bool,
    pub(super) groups: std::sync::Mutex<Groups>,
    pub(super) started: std::sync::Mutex<Vec<(String, Vec<String>)>>,
    pub(super) stops: std::sync::Mutex<Vec<String>>,
}

impl FakeSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every disk is this full.
    pub fn with_disk(mut self, size: u64, used: u64) -> Self {
        self.disk = Some(DiskUsage { size, used });
        self
    }

    pub fn with_service(self, unit: &str, enabled: &str, active: &str) -> Self {
        self.set_service(unit, enabled, active);
        self
    }

    pub(super) fn set_service(&self, unit: &str, enabled: &str, active: &str) {
        if let Ok(mut services) = self.services.lock() {
            services.insert(
                unit.to_owned(),
                ServiceState {
                    enabled: Some(enabled.to_owned()),
                    active: Some(active.to_owned()),
                },
            );
        }
    }

    /// What the unit file of `unit` says about it.
    pub fn with_unit_file(mut self, unit: &str, description: &str, timer: &[(&str, &str)]) -> Self {
        self.details.insert(
            unit.to_owned(),
            UnitDetails {
                file: Some(format!("/usr/lib/systemd/system/{unit}")),
                description: Some(description.to_owned()),
                timer: timer
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            },
        );
        self
    }

    pub fn at(mut self, time: &str) -> Self {
        self.time = Some(time.to_owned());
        self
    }

    /// Starting a background job is refused, as it is without administrator rights.
    pub fn refusing_jobs(mut self) -> Self {
        self.refuse_jobs = true;
        self
    }

    /// How the disk counts room (default: not at all).
    pub fn with_quotas(self, mode: QuotaMode) -> Self {
        if let Ok(mut quotas) = self.quotas.lock() {
            *quotas = Some(mode);
        }
        self
    }

    /// Another snapall is making a change.
    pub fn busy_with(mut self, doing: &str) -> Self {
        self.busy = Some(Busy {
            pid: 4242,
            doing: doing.to_owned(),
            since: Some("12:03:00".to_owned()),
        });
        self
    }

    /// Groups the person already made.
    pub fn with_groups(self, groups: Groups) -> Self {
        if let Ok(mut held) = self.groups.lock() {
            *held = groups;
        }
        self
    }

    /// This machine can hand a change to a process that goes on without the window.
    pub fn running_in_the_background(mut self) -> Self {
        self.background = true;
        self
    }

    /// What was handed to the background, in order: the change's name and the arguments.
    pub fn handed_to_the_background(&self) -> Vec<(String, Vec<String>)> {
        self.started.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// The changes that were asked to stop, in order.
    pub fn stops_asked(&self) -> Vec<String> {
        self.stops.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// An event already in the journal, as if an earlier snapall had written it. A process id of 0 is a
    /// process that is gone.
    pub fn with_event(self, event: journal::Event) -> Self {
        if let Ok(mut journal) = self.journal.lock() {
            journal.push(event);
        }
        self
    }

    /// Everything recorded so far, in order.
    pub fn events(&self) -> Vec<journal::Event> {
        self.journal.lock().map(|j| j.clone()).unwrap_or_default()
    }

    /// Everything done to jobs, in order.
    pub fn job_actions(&self) -> Vec<(String, JobAction)> {
        self.actions.lock().map(|a| a.clone()).unwrap_or_default()
    }

    /// The jobs that were switched on, in order.
    pub fn jobs_switched_on(&self) -> Vec<String> {
        self.job_actions()
            .into_iter()
            .filter(|(_, action)| *action == JobAction::EnableNow)
            .map(|(unit, _)| unit)
            .collect()
    }

    /// The plugin that keeps the boot menu in step with the snapshots is installed.
    pub fn with_boot_menu_plugin(mut self) -> Self {
        self.boot_menu_plugin = true;
        self
    }
}
