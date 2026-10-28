// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A machine whose answers a test sets: its disk, its jobs, its clock, who else is making a change.

use std::collections::BTreeMap;

use crate::keeping::groups::Groups;
use crate::keeping::jobs::JobAction;
use crate::looking::journal;
use crate::ports::machine::{
    Busy, DiskUsage, Hold, QuotaMode, ServiceState, SystemProbe, UnitDetails,
};

/// A machine whose answers are set by the test: how full its disk is, which jobs run, what time it is,
/// how room is counted, who else is making a change.
#[derive(Default)]
pub struct FakeSystem {
    disk: Option<DiskUsage>,
    services: std::sync::Mutex<BTreeMap<String, ServiceState>>,
    details: BTreeMap<String, UnitDetails>,
    time: Option<String>,
    files: Vec<String>,
    refuse_jobs: bool,
    actions: std::sync::Mutex<Vec<(String, JobAction)>>,
    quotas: std::sync::Mutex<Option<QuotaMode>>,
    busy: Option<Busy>,
    journal: std::sync::Mutex<Vec<journal::Event>>,
    background: bool,
    groups: std::sync::Mutex<Groups>,
    started: std::sync::Mutex<Vec<(String, Vec<String>)>>,
    stops: std::sync::Mutex<Vec<String>>,
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

    fn set_service(&self, unit: &str, enabled: &str, active: &str) {
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

    pub fn with_file(mut self, path: &str) -> Self {
        self.files.push(path.to_owned());
        self
    }
}

impl SystemProbe for FakeSystem {
    fn disk_usage(&self, _path: &str) -> Option<DiskUsage> {
        self.disk
    }

    fn service(&self, unit: &str) -> ServiceState {
        let known = self
            .services
            .lock()
            .ok()
            .and_then(|services| services.get(unit).cloned());
        known.unwrap_or(ServiceState {
            enabled: Some("not-found".to_owned()),
            active: Some("inactive".to_owned()),
        })
    }

    fn local_time(&self) -> Option<String> {
        self.time.clone()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.files.iter().any(|f| f == path)
    }

    fn unit_details(&self, unit: &str) -> Option<UnitDetails> {
        self.details.get(unit).cloned()
    }

    fn control_job(&self, unit: &str, action: JobAction) -> Result<(), String> {
        use crate::keeping::jobs::JobAction::{DisableNow, EnableNow, Restart, Start, Stop};
        if self.refuse_jobs {
            return Err("Access denied".to_owned());
        }
        if let Ok(mut actions) = self.actions.lock() {
            actions.push((unit.to_owned(), action));
        }
        // as the real thing: each action leaves the job in the state it names
        let current = self.service(unit);
        let enabled = current
            .enabled
            .clone()
            .unwrap_or_else(|| "disabled".to_owned());
        let (enabled, active) = match action {
            EnableNow => ("enabled".to_owned(), "active"),
            DisableNow => ("disabled".to_owned(), "inactive"),
            Start | Restart => (enabled, "active"),
            Stop => (enabled, "inactive"),
        };
        self.set_service(unit, &enabled, active);
        Ok(())
    }

    fn quota_mode(&self, _path: &str) -> QuotaMode {
        self.quotas
            .lock()
            .ok()
            .and_then(|mode| *mode)
            .unwrap_or(QuotaMode::Off)
    }

    fn set_quotas(&self, _path: &str, on: bool) -> Result<(), String> {
        if self.refuse_jobs {
            return Err("Access denied".to_owned());
        }
        if let Ok(mut quotas) = self.quotas.lock() {
            *quotas = Some(if on { QuotaMode::Full } else { QuotaMode::Off });
        }
        Ok(())
    }

    fn hold_changes(&self, _doing: &str) -> Result<Hold, Busy> {
        match &self.busy {
            Some(busy) => Err(busy.clone()),
            None => Ok(Box::new(())),
        }
    }

    fn groups(&self) -> Result<Groups, String> {
        self.groups
            .lock()
            .map(|g| g.clone())
            .map_err(|e| e.to_string())
    }

    fn save_groups(&self, groups: &Groups) -> Result<(), String> {
        if self.refuse_jobs {
            return Err("this needs administrator rights: start snapall with sudo".to_owned());
        }
        *self.groups.lock().map_err(|e| e.to_string())? = groups.clone();
        Ok(())
    }

    fn can_run_in_background(&self) -> bool {
        self.background
    }

    fn start_in_background(&self, id: &str, arguments: &[String]) -> Result<(), String> {
        if self.refuse_jobs {
            return Err("Access denied".to_owned());
        }
        if let Ok(mut started) = self.started.lock() {
            started.push((id.to_owned(), arguments.to_vec()));
        }
        Ok(())
    }

    fn request_stop(&self, id: &str) -> Result<(), String> {
        if let Ok(mut stops) = self.stops.lock() {
            stops.push(id.to_owned());
        }
        Ok(())
    }

    fn stop_requested(&self, id: &str) -> bool {
        self.stops
            .lock()
            .is_ok_and(|stops| stops.iter().any(|stop| stop == id))
    }

    fn record(&self, event: &journal::Event) {
        if let Ok(mut journal) = self.journal.lock() {
            journal.push(event.clone());
        }
    }

    fn history(&self, limit: usize) -> Vec<journal::Entry> {
        let events = self.events();
        let mut entries = journal::fold(&events, |pid| pid != 0);
        entries.truncate(limit);
        entries
    }
}
