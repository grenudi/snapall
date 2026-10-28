// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The questions the window asks, and what each one holds while it is open.

use super::*;
use crate::model::*;

/// Where a restore prompt is: looking at what would change, ready, or unable to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreStage {
    Loading,
    Ready(Preview),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    One(AreaName),
    All,
    /// The areas of a group the person made.
    Group(snapall_core::GroupName),
}

/// The editor of how many snapshots an area keeps and when its oldest go: what is being set, and what
/// that would do to the snapshots there are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitsEditor {
    pub area: AreaName,
    /// What snapper has now.
    pub before: snapall_core::Limits,
    /// What is being set.
    pub limits: snapall_core::Limits,
    /// Which of the three is chosen: 0 how many, 1 how young, 2 whether cleanup runs.
    pub field: usize,
    /// The area's snapshots, for the preview, once they have arrived.
    pub snapshots: Option<Vec<snapall_core::SnapshotInfo>>,
    pub error: Option<String>,
}

/// The prompt before an area stops being protected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopProtecting {
    pub area: AreaName,
    /// What it would do, or why it would not; `None` while it is being worked out.
    pub plan: Option<Result<snapall_core::protecting::unprotect::Plan, String>>,
    /// What has been typed so far, to be the area's name.
    pub typed: String,
    pub error: Option<String>,
}

/// The question before a change: what it will do, and the answer that goes with its weight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    pub act: snapall_core::Act,
    /// What the answer must name, where the change is done to one thing (`destroy home`).
    pub target: Option<String>,
    /// What it will do, in plain words; the first line is the one that matters most.
    pub lines: Vec<String>,
    pub typed: String,
    pub error: Option<String>,
    /// What starts when it is answered.
    pub then: Vec<Effect>,
    /// Where to go back to if it is cancelled, instead of closing.
    pub on_cancel: Option<Box<Modal>>,
    /// Where to be after it is answered, instead of closing.
    pub on_done: Option<Box<Modal>>,
}

/// Where the groups prompt is: the list of them, naming a new one, or choosing the areas of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupsScreen {
    /// `all` first, then the groups the person made; one is chosen.
    List { cursor: usize },
    /// Typing the name of a new group.
    Name {
        input: String,
        error: Option<String>,
    },
    /// Choosing which areas are in the group.
    Pick {
        name: snapall_core::GroupName,
        picks: Vec<(AreaName, bool)>,
        cursor: usize,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Describe {
        target: Target,
        input: String,
        error: Option<String>,
    },
    ConfirmDelete {
        area: AreaName,
        ids: Vec<SnapshotId>,
    },
    /// Put an area back to how a snapshot had it.
    Restore {
        area: AreaName,
        id: SnapshotId,
        /// One path of the area, or `None` for all of it.
        path: Option<String>,
        stage: RestoreStage,
        /// The restore point this snapshot belongs to, once looked up, when it has one with more than one area.
        offer: Option<PointOffer>,
    },
    /// The explanation of what snapshots are (page 0) and of the keys (page 1).
    Help { page: usize },
    /// Offering to protect one of the parts of the disk that have no snapshots yet.
    Protect { index: usize },
    /// The restore points, one chosen: what putting it back would do, and the way to do it.
    Points {
        index: usize,
        /// Every restore point, newest first, once looked up.
        points: Option<Vec<snapall_core::RestorePoint>>,
        /// What putting the chosen one back would do, once looked up.
        preview: Option<PointOffer>,
    },
    /// One background job: what it is, how to check on it, and the keys that control it.
    Job { unit: String },
    /// Asking before switching the counting of room on or off for the disk.
    Quotas { turn_on: bool },
    /// The groups of areas: see them, make one, change one, take one away.
    Groups(GroupsScreen),
    /// How many snapshots an area keeps, and when its oldest are removed.
    Limits(LimitsEditor),
    /// Stopping the protection of an area, which deletes its snapshots for good.
    StopProtecting(StopProtecting),
    /// The question before a change.
    Confirm(Confirm),
    /// Asking before a running change is asked to stop.
    StopChange { id: String, what: String },
}
