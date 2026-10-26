// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window knows: the lists, the cursor, the open prompt, the message, and what the shell told it about the machine.

use crate::model::*;

/// Something that is fetched on demand, so "not asked yet", "here" and "failed" are
/// distinct states instead of an empty list standing in for all three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load<T> {
    NotLoaded,
    Loaded(T),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub name: ConfigName,
    /// Oldest first, as the port returns them. The view shows newest first.
    pub snapshots: Load<Vec<SnapshotInfo>>,
}

/// One file the UI was opened on: where it is, and the config whose snapshots hold its history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTarget {
    pub path: String,
    pub config: ConfigName,
}

/// The file history screen's own state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileView {
    pub target: FileTarget,
    /// Oldest first, as the backend gives them; shown newest first.
    pub versions: Load<Vec<FileVersion>>,
    pub cursor: usize,
}

/// The restore point a snapshot belongs to, and what putting all of it back would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointOffer {
    pub point: snapall_core::RestorePoint,
    pub views: Vec<snapall_core::MemberPreview>,
}

impl PointOffer {
    /// How many areas would change.
    pub fn areas_to_change(&self) -> usize {
        self.views
            .iter()
            .filter(|v| {
                matches!(&v.view, snapall_core::MemberView::Changes(p) if !p.changes.is_empty())
            })
            .count()
    }
}

/// What putting a restore point back did, boiled down to what a person is told.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PointOutcome {
    pub restored: usize,
    pub unchanged: usize,
    pub failed: Vec<String>,
    /// The restore point of the safety snapshots: restoring it undoes everything, if anything changed.
    pub undo: Option<String>,
}

/// A part of the disk that has no snapshots yet, offered for protecting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// Where it is mounted: `/home`.
    pub mount_point: String,
    /// What a person calls it: `Home`.
    pub title: String,
    /// What protecting it means, in plain words: what it holds, whether it is worth it, what will and
    /// will not happen.
    pub summary: String,
    pub recommended: bool,
}

/// What the UI is told about the machine it runs on, once, at the start. The model never reads
/// the machine; the shell does, and hands over plain text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiContext {
    /// A one-line tip (for example that snapshots cannot be booted), shown until something else
    /// needs the line.
    pub boot_tip: Option<String>,
    /// How to restore the *system itself*, shown instead of a confirm button for the system root.
    pub restore_route: String,
    /// Open on this file's history instead of on all configs.
    pub file: Option<FileTarget>,
    /// The parts of the disk that have no snapshots yet and could have, best first.
    pub offers: Vec<Offer>,
    /// Whether snapall may change how snapshots are kept (it was started with `sudo`).
    pub administrator: bool,
    /// One line for the footer when something worth protecting is not: it says what to press.
    pub protect_hint: Option<String>,
}

/// Where a restore prompt is: looking at what would change, ready, or unable to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreStage {
    Loading,
    Ready(Preview),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    One(ConfigName),
    All,
    /// The areas of a group the person made.
    Group(snapall_core::GroupName),
}

/// The editor of how many snapshots an area keeps and when its oldest go: what is being set, and what
/// that would do to the snapshots there are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitsEditor {
    pub config: ConfigName,
    /// What snapper has now.
    pub before: snapall_core::Limits,
    /// What is being set.
    pub limits: snapall_core::Limits,
    /// Which of the three is chosen: 0 how many, 1 how young, 2 whether cleanup runs.
    pub field: usize,
    /// The area's snapshots, for the preview, once they have arrived.
    pub snapshots: Option<Vec<snapall_core::SnapshotInfo>>,
    /// Enter was pressed on a change that would remove snapshots: the next one applies it.
    pub armed: bool,
    pub error: Option<String>,
}

/// The prompt before an area stops being protected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopProtecting {
    pub config: ConfigName,
    /// What it would do, or why it would not; `None` while it is being worked out.
    pub plan: Option<Result<snapall_core::unprotect::Plan, String>>,
    /// What has been typed so far, to be the area's name.
    pub typed: String,
    pub error: Option<String>,
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
        picks: Vec<(ConfigName, bool)>,
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
        config: ConfigName,
        ids: Vec<SnapshotId>,
    },
    /// Put a config back to how a snapshot had it.
    Restore {
        config: ConfigName,
        id: SnapshotId,
        /// One path of the config, or `None` for all of it.
        path: Option<String>,
        stage: RestoreStage,
    },
    /// The explanation of what snapshots are (page 0) and of the keys (page 1).
    Help { page: usize },
    /// Offering to protect one of the parts of the disk that have no snapshots yet.
    Protect { index: usize },
    /// The restore points, one chosen: what putting it back would do, and the way to do it.
    Points { index: usize },
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
    /// Asking before a running change is asked to stop.
    StopChange { id: String, what: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Info(String),
    Error(String),
}

/// A change this window handed to the background: its name, and how many times the journal was read
/// without it appearing there (a change that never reports is eventually let go of).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watched {
    pub id: String,
    pub unseen: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub configs: Load<Vec<ConfigEntry>>,
    pub config_cursor: usize,
    /// Index into the *displayed* (newest-first) snapshot list.
    pub snapshot_cursor: usize,
    pub marked: BTreeSet<SnapshotId>,
    pub focus: Pane,
    pub modal: Option<Modal>,
    pub notice: Option<Notice>,
    pub quit: bool,
    /// Effects handed to the shell whose results have not come back. While it is above zero the
    /// screen says "working" and the keys that would start more work are refused. A shell that
    /// runs effects instantly (the tests) never sees it above zero between messages.
    pub pending: usize,
    /// Advances on every `Tick`, to animate the working indicator.
    pub spinner: usize,
    /// The report of how protection stands has been asked for once the first screen had its data.
    pub health_asked: bool,
    /// The groups of areas the person made, and why they could not be read, if they could not.
    pub groups: snapall_core::Groups,
    pub groups_error: Option<String>,
    /// Changes this window handed to the background and has not seen end: it follows them, and looks
    /// at the lists again when they have.
    pub watching: Vec<Watched>,
    /// Ticks since the journal was last looked at while something runs.
    pub poll_ticks: u32,
    /// What the changes in flight are, in the present tense: what the screen says is being done.
    pub doing: Vec<String>,
    /// How many ticks (a tenth of a second each) work has been in flight without a pause.
    pub busy_ticks: u32,
    /// What the shell told the UI about the machine, once, at the start.
    pub context: UiContext,
    /// Set when the UI was opened on one file: the screen is then that file's history.
    pub file: Option<FileView>,
    /// The button the mouse is over: what it does is written on the footer line while it is.
    pub hover: Option<Button>,
    /// The parts of the disk still offered for protecting; one leaves the list when it is protected.
    pub offers: Vec<Offer>,
    /// A message to show once the list has been reloaded: the reload clears older notices, and a
    /// confirmation must not be one of them.
    pub after_reload: Option<Notice>,
    /// The restore point of the snapshot the restore prompt is about, once looked up.
    pub point_offer: Option<(ConfigName, SnapshotId, PointOffer)>,
    /// The restore points, newest first, once looked up (`None` while looking).
    pub points: Option<Vec<snapall_core::RestorePoint>>,
    /// What putting the chosen restore point back would do.
    pub point_preview: Option<PointOffer>,
    /// Which view is showing.
    pub tab: Tab,
    pub health: HealthTab,
    pub activity: ActivityTab,
}

impl Default for State {
    fn default() -> Self {
        Self {
            configs: Load::NotLoaded,
            config_cursor: 0,
            snapshot_cursor: 0,
            marked: BTreeSet::new(),
            focus: Pane::Configs,
            modal: None,
            notice: None,
            quit: false,
            pending: 0,
            spinner: 0,
            health_asked: false,
            groups: snapall_core::Groups::default(),
            groups_error: None,
            watching: Vec::new(),
            poll_ticks: 0,
            doing: Vec::new(),
            busy_ticks: 0,
            context: UiContext::default(),
            file: None,
            hover: None,
            offers: Vec::new(),
            after_reload: None,
            point_offer: None,
            points: None,
            point_preview: None,
            tab: Tab::Snapshots,
            health: HealthTab::default(),
            activity: ActivityTab::default(),
        }
    }
}

impl State {
    /// Whether something is going on that the window must keep an eye on: work in flight, a change it
    /// handed to the background, or anything the journal says is running. The clock ticks while it is so.
    pub fn is_following_something(&self) -> bool {
        self.pending > 0
            || !self.watching.is_empty()
            || self
                .activity
                .entries
                .iter()
                .any(|entry| entry.status == snapall_core::JournalStatus::Running)
    }

    pub fn with_context(context: UiContext) -> Self {
        let file = context.file.clone().map(|target| FileView {
            target,
            versions: Load::NotLoaded,
            cursor: 0,
        });
        Self {
            offers: context.offers.clone(),
            context,
            file,
            ..Self::default()
        }
    }

    /// The versions of the open file, newest first.
    pub fn displayed_versions(&self) -> Vec<&FileVersion> {
        match self.file.as_ref().map(|view| &view.versions) {
            Some(Load::Loaded(list)) => list.iter().rev().collect(),
            _ => Vec::new(),
        }
    }

    pub fn entries(&self) -> &[ConfigEntry] {
        match &self.configs {
            Load::Loaded(entries) => entries,
            _ => &[],
        }
    }

    pub fn selected_config(&self) -> Option<&ConfigEntry> {
        self.entries().get(self.config_cursor)
    }

    /// The selected config's snapshots, newest first.
    pub fn displayed_snapshots(&self) -> Vec<&SnapshotInfo> {
        match self.selected_config().map(|c| &c.snapshots) {
            Some(Load::Loaded(list)) => list.iter().rev().collect(),
            _ => Vec::new(),
        }
    }

    pub fn selected_snapshot(&self) -> Option<&SnapshotInfo> {
        self.displayed_snapshots()
            .get(self.snapshot_cursor)
            .copied()
    }

    pub(crate) fn config_names(&self) -> Vec<ConfigName> {
        self.entries().iter().map(|e| e.name.clone()).collect()
    }

    pub(crate) fn entry_mut(&mut self, name: &ConfigName) -> Option<&mut ConfigEntry> {
        match &mut self.configs {
            Load::Loaded(entries) => entries.iter_mut().find(|e| &e.name == name),
            _ => None,
        }
    }
}
