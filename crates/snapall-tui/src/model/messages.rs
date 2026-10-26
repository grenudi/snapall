// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window asks the shell to do (effects) and what the shell reports back (messages).

use crate::model::*;

/// What the shell should do. Each effect is answered by exactly one [`Msg`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    LoadConfigs,
    LoadSnapshots(ConfigName),
    Create {
        targets: Vec<ConfigName>,
        description: Description,
    },
    Delete {
        config: ConfigName,
        ids: Vec<SnapshotId>,
    },
    /// What would restoring `id` change?
    LoadChanges {
        config: ConfigName,
        id: SnapshotId,
        path: Option<String>,
    },
    /// Start keeping snapshots of the part of the disk at `mount_point`.
    Protect {
        mount_point: String,
    },
    /// Is this snapshot part of a restore point, and what would putting all of it back do?
    LoadPointOffer {
        config: ConfigName,
        id: SnapshotId,
    },
    /// Put every area of the restore point back.
    RestorePoint {
        point: snapall_core::RestorePoint,
    },
    /// Look at how protection stands: the report the status bar and the Health tab are made from.
    LoadHealth,
    /// Do something to a background job (a unit name from the catalogue).
    ControlJob {
        unit: String,
        action: snapall_core::JobAction,
    },
    /// Switch counting the room snapshots take on or off, for the disks holding these paths.
    SetQuotas {
        turn_on: bool,
        paths: Vec<String>,
    },
    /// Switch automatic cleanup on.
    SwitchOnCleanup,
    /// Look up every restore point.
    LoadPoints,
    /// What would putting this restore point back do?
    LoadPointPreview {
        point: snapall_core::RestorePoint,
    },
    /// Put `config` back to how `id` had it (after a safety snapshot).
    Restore {
        config: ConfigName,
        id: SnapshotId,
        path: Option<String>,
    },
    /// How the file stood in each snapshot of its config.
    LoadFileVersions {
        config: ConfigName,
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// The UI has opened: load the configs.
    Start,
    /// Time passed while work is in flight; animates the indicator.
    Tick,
    Key(Key),
    /// The mouse moved over a button (or off every button).
    Hover(Option<Button>),
    /// A left click, resolved by the view.
    Click(Click),
    /// The wheel over a pane: positive moves down.
    Scroll {
        pane: Pane,
        delta: i32,
    },
    ChangesLoaded {
        config: ConfigName,
        id: SnapshotId,
        result: Result<Preview, String>,
    },
    Restored {
        config: ConfigName,
        id: SnapshotId,
        path: Option<String>,
        result: Result<Restored, String>,
    },
    FileVersionsLoaded {
        config: ConfigName,
        path: String,
        result: Result<Vec<FileVersion>, String>,
    },
    ConfigsLoaded(Result<Vec<ConfigName>, String>),
    /// The restore point of a snapshot, if it has one with more than one area.
    PointOfferLoaded {
        config: ConfigName,
        id: SnapshotId,
        offer: Option<PointOffer>,
    },
    /// A background job was controlled, or why it could not be.
    JobControlled {
        unit: String,
        action: snapall_core::JobAction,
        result: Result<(), String>,
    },
    /// Counting room was switched, or why it could not be.
    QuotasSet {
        turn_on: bool,
        result: Result<(), String>,
    },
    /// A change was not made because another snapall is making one; the reason says who.
    Refused(String),
    /// How protection stands, or why that could not be found out.
    HealthLoaded(Result<Box<HealthData>, String>),
    /// Automatic cleanup was switched on, or why it could not be.
    CleanupSwitched(Result<(), String>),
    /// What the shell did for a message that changed something: what it was, how it went, when and
    /// for how long. The message it describes is inside, and is handled as if it had arrived alone.
    Finished(Box<Finished>),
    /// Every restore point, or why they could not be looked up.
    PointsLoaded(Result<Vec<snapall_core::RestorePoint>, String>),
    /// What putting one restore point back would do.
    PointPreviewLoaded(PointOffer),
    /// A restore point was put back.
    PointRestored {
        point: String,
        result: Result<PointOutcome, String>,
    },
    /// A part of the disk was protected, or why it could not be (in plain words).
    Protected {
        mount_point: String,
        result: Result<ConfigName, String>,
    },
    SnapshotsLoaded {
        config: ConfigName,
        result: Result<Vec<SnapshotInfo>, String>,
    },
    Created(Vec<(ConfigName, Result<SnapshotId, String>)>),
    Deleted {
        config: ConfigName,
        results: Vec<(SnapshotId, Result<(), String>)>,
    },
}

/// Apply one message. Pure: the only output besides the mutation is the effect list.
///
/// Every effect returned counts as pending until its result message arrives; every result
/// message settles one. That bookkeeping lives here, not in the shell, so it is tested like
/// everything else.
/// A change that was made, as the shell reports it: for the Activity tab, around the message that says
/// what came of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub what: String,
    /// The same, in the present tense, as it was announced while it was being done.
    pub present: String,
    pub outcome: Result<String, String>,
    pub took: Duration,
    pub at: Option<String>,
    pub msg: Msg,
}
