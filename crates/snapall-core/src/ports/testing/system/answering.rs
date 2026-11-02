// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How the stand-in answers what snapall asks of the machine.

use super::*;
use crate::keeping::groups::Groups;
use crate::keeping::jobs::JobAction;
use crate::looking::journal;
use crate::ports::machine::{
    Busy, Counting, DiskUsage, Hold, QuotaMode, ServiceState, SystemProbe, UnitDetails,
};

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

    fn set_quotas(&self, _path: &str, to: Counting) -> Result<(), String> {
        if self.refuse_jobs {
            return Err("Access denied".to_owned());
        }
        if let Ok(mut quotas) = self.quotas.lock() {
            *quotas = Some(if to.is_on() {
                QuotaMode::Full
            } else {
                QuotaMode::Off
            });
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
