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
    fn control_job(&self, unit: &str, action: crate::services::JobAction) -> Result<(), String>;

    /// How the disk holding `path` counts the room snapshots take.
    fn quota_mode(&self, path: &str) -> QuotaMode;

    /// Switch room counting on or off for the disk holding `path`. It applies to the whole disk, not
    /// to one area, and needs administrator rights.
    fn set_quotas(&self, path: &str, on: bool) -> Result<(), String>;

    /// Take the right to make a change; `Busy` says who has it. Reading never needs it.
    fn hold_changes(&self, doing: &str) -> Result<Hold, Busy>;

    /// The groups of areas the person made. None made, or no file, is an empty set; a file that cannot be
    /// read is an error that says where.
    fn groups(&self) -> Result<crate::groups::Groups, String> {
        Ok(crate::groups::Groups::default())
    }

    /// Keep the groups. It needs administrator rights, and says so in plain words when it lacks them.
    fn save_groups(&self, _groups: &crate::groups::Groups) -> Result<(), String> {
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

    /// Remember an event of a change (see [`crate::journal`]). A machine that cannot remember says
    /// nothing; failing to remember never stops a change.
    fn record(&self, _event: &crate::journal::Event) {}

    /// What snapall has done, newest first, whichever snapall did it; at most `limit` changes.
    fn history(&self, _limit: usize) -> Vec<crate::journal::Entry> {
        Vec::new()
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
        crate::services::schedule_words(&self.timer)
    }
}

/// How a disk counts the room snapshots take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaMode {
    /// Not counted: the room a snapshot holds is not known.
    Off,
    /// Fully counted: what each snapshot alone holds is known, at a cost when snapshots are deleted.
    Full,
    /// The cheap kind (kernel 6.7 and later): room is charged to whoever wrote it first, so what a
    /// snapshot holds is not known either.
    Simple,
    /// Could not be found out.
    Unknown,
}

/// What asking for room counting to be switched on or off comes to, given how the disk counts it now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switching {
    /// It is already that way: nothing to do.
    AlreadyThatWay,
    /// The simple kind cannot be changed to the full kind in place.
    CannotBeDoneInPlace,
    Possible,
}

impl QuotaMode {
    /// How the disk counts, in a few words: `off`, `on`, `on, the simple kind`, or that it could not be found out.
    pub fn word(self) -> &'static str {
        match self {
            QuotaMode::Off => "off",
            QuotaMode::Full => "on",
            QuotaMode::Simple => "on, the simple kind",
            QuotaMode::Unknown => "could not be found out",
        }
    }

    pub fn switching(self, on: bool) -> Switching {
        match (self, on) {
            (QuotaMode::Simple, true) => Switching::CannotBeDoneInPlace,
            (QuotaMode::Full, true) | (QuotaMode::Off, false) => Switching::AlreadyThatWay,
            (QuotaMode::Off, true)
            | (QuotaMode::Full, false)
            | (QuotaMode::Simple, false)
            | (QuotaMode::Unknown, _) => Switching::Possible,
        }
    }
}

/// Another snapall is making a change, so this one must wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Busy {
    pub pid: u32,
    /// What it is doing, as it said when it started.
    pub doing: String,
    /// When it started, if known.
    pub since: Option<String>,
}

impl std::fmt::Display for Busy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Another snapall is making a change right now ({}",
            self.doing
        )?;
        if let Some(since) = &self.since {
            write!(f, ", since {since}")?;
        }
        write!(
            f,
            ", process {}). Wait for it to finish, then try again.",
            self.pid
        )
    }
}

/// Held while a change is made; letting go of it lets the next one start.
pub type Hold = Box<dyn std::any::Any + Send>;

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

    fn control_job(&self, _unit: &str, _action: crate::services::JobAction) -> Result<(), String> {
        Err("this computer cannot be asked to control background jobs".to_owned())
    }

    fn quota_mode(&self, _path: &str) -> QuotaMode {
        QuotaMode::Unknown
    }

    fn set_quotas(&self, _path: &str, _on: bool) -> Result<(), String> {
        Err("this computer cannot be asked to change how room is counted".to_owned())
    }

    fn hold_changes(&self, _doing: &str) -> Result<Hold, Busy> {
        Ok(Box::new(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_room_counting_is_already_so_impossible_in_place_or_possible() {
        use QuotaMode::{Full, Off, Simple, Unknown};
        assert_eq!(Full.switching(true), Switching::AlreadyThatWay);
        assert_eq!(Off.switching(false), Switching::AlreadyThatWay);
        assert_eq!(Simple.switching(true), Switching::CannotBeDoneInPlace);
        for (mode, on) in [
            (Off, true),
            (Full, false),
            (Simple, false),
            (Unknown, true),
            (Unknown, false),
        ] {
            assert_eq!(mode.switching(on), Switching::Possible, "{mode:?} to {on}");
        }
    }
}
