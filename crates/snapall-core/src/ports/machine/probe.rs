// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall asks of the machine, as one trait, and the stand-in that answers nothing.

use super::*;

pub trait SystemProbe {
    /// The disk that holds `path`, or `None` if it cannot be read.
    fn disk_usage(&self, path: &str) -> Option<DiskUsage>;

    /// The state of a background job (a systemd unit such as `snapper-cleanup.timer`).
    fn service(&self, unit: &str) -> ServiceState;

    /// The local time as snapper writes dates (`2026-10-25 18:15:00`): the zone snapper's dates are
    /// in, so the two can be compared.
    fn local_time(&self) -> Option<String>;

    fn file_exists(&self, path: &str) -> bool;

    /// What a job's unit file says about it, if it can be found.
    fn unit_details(&self, unit: &str) -> Option<UnitDetails>;

    /// Do something to a background job. It needs administrator rights, and says why in plain words
    /// when it cannot.
    fn control_job(
        &self,
        unit: &str,
        action: crate::keeping::jobs::JobAction,
    ) -> Result<(), String>;

    /// How the disk holding `path` counts the room snapshots take.
    fn quota_mode(&self, path: &str) -> QuotaMode;

    /// Switch room counting on or off for the disk holding `path`. It applies to the whole disk, not
    /// to one area, and needs administrator rights.
    fn set_quotas(&self, path: &str, to: Counting) -> Result<(), String>;

    /// Take the right to make a change; `Busy` says who has it. Reading never needs it.
    fn hold_changes(&self, doing: &str) -> Result<Hold, Busy>;

    /// The groups of areas the person made. None made, or no file, is an empty set; a file that cannot be
    /// read is an error that says where.
    fn groups(&self) -> Result<crate::keeping::groups::Groups, String> {
        Ok(crate::keeping::groups::Groups::default())
    }

    /// Keep the groups. It needs administrator rights, and says so in plain words when it lacks them.
    fn save_groups(&self, _groups: &crate::keeping::groups::Groups) -> Result<(), String> {
        Err("this computer cannot keep groups".to_owned())
    }

    /// Whether a change can be handed to a process that goes on without this one.
    fn can_run_in_background(&self) -> bool {
        false
    }

    /// Run `snapall` with these arguments, detached from this program: it goes on if this program ends.
    /// `id` names the change in the journal; the program given the arguments writes its events itself.
    fn start_in_background(&self, _id: &str, _arguments: &[String]) -> Result<(), String> {
        Err("this computer cannot run changes in the background".to_owned())
    }

    /// Ask the change `id` to stop after the step it is on. It is only a request: the change looks for it
    /// between its steps, never in the middle of one.
    fn request_stop(&self, _id: &str) -> Result<(), String> {
        Err("this computer cannot be asked to stop a change".to_owned())
    }

    /// The request to stop `id` is answered, or moot: forget it.
    fn clear_stop(&self, _id: &str) {}

    /// Whether a stop was asked of the change `id`.
    fn stop_requested(&self, _id: &str) -> bool {
        false
    }

    /// Remember an event of a change (see [`crate::looking::journal`]). A machine that cannot remember says
    /// nothing; failing to remember never stops a change.
    fn record(&self, _event: &crate::looking::journal::Event) {}

    /// What snapall has done, newest first, whichever snapall did it; at most `limit` changes.
    fn history(&self, _limit: usize) -> Vec<crate::looking::journal::Entry> {
        Vec::new()
    }
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

    fn unit_details(&self, _unit: &str) -> Option<UnitDetails> {
        None
    }

    fn control_job(
        &self,
        _unit: &str,
        _action: crate::keeping::jobs::JobAction,
    ) -> Result<(), String> {
        Err("this computer cannot be asked to control background jobs".to_owned())
    }

    fn quota_mode(&self, _path: &str) -> QuotaMode {
        QuotaMode::Unknown
    }

    fn set_quotas(&self, _path: &str, _to: Counting) -> Result<(), String> {
        Err("this computer cannot be asked to change how room is counted".to_owned())
    }

    fn hold_changes(&self, _doing: &str) -> Result<Hold, Busy> {
        Ok(Box::new(()))
    }
}
