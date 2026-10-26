// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure core of the UI: state, messages, effects and `update`.
//!
//! Nothing here touches a terminal or a backend. `update` mutates the state and returns
//! the *effects* the shell should perform; the shell reports what happened as new messages.

use std::collections::BTreeSet;
use std::time::Duration;

use crate::tab::Tab;
use crate::tabs::activity::{Activity, ActivityTab};
use crate::tabs::health::{HealthData, HealthTab};
use snapall_core::{
    ConfigName, Description, FileState, FileVersion, Preview, Restored, SnapshotId, SnapshotInfo,
};

/// A key, independent of any terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Tab,
    Enter,
    Esc,
    Backspace,
    CtrlC,
    Char(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Configs,
    Snapshots,
}

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

/// Something that can be clicked. Each is the mouse spelling of a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Create,
    CreateAll,
    Delete,
    Restore,
    Reload,
    Help,
    Quit,
    /// The affirmative button of a prompt (Enter).
    Confirm,
    /// The negative button of a prompt (Esc).
    Cancel,
    /// See what has no snapshots yet and protect it.
    Protect,
    /// In a prompt that shows several things one at a time: the next one.
    Next,
    /// In the restore prompt: put every area of the snapshot's restore point back.
    RestoreAll,
    /// See the restore points and put every area back to one of them.
    Points,
}

impl Button {
    /// The key this button stands for, so a click goes through exactly the code the key does.
    pub fn key(self) -> Key {
        match self {
            Button::Create => Key::Char('c'),
            Button::CreateAll => Key::Char('a'),
            Button::Delete => Key::Char('d'),
            Button::Restore => Key::Char('u'),
            Button::Reload => Key::Char('r'),
            Button::Help => Key::Char('?'),
            Button::Quit => Key::Char('q'),
            Button::Confirm => Key::Enter,
            Button::Cancel => Key::Esc,
            Button::Protect => Key::Char('p'),
            Button::Next => Key::Tab,
            Button::RestoreAll => Key::Char('A'),
            Button::Points => Key::Char('R'),
        }
    }
}

/// A left click, already resolved by the view to what is under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Button(Button),
    /// A row of a pane (its position in what is displayed, not on screen).
    Row {
        pane: Pane,
        index: usize,
    },
    /// The mark column of a snapshot row: toggles the mark without moving the cursor.
    Mark {
        index: usize,
    },
    /// A name in the tab bar.
    Tab(Tab),
    /// An item of the status bar: goes to the Health tab, at the row that explains it.
    Chip(snapall_core::Topic),
    /// A row of the Health or Activity tab, whichever is showing.
    TabRow {
        index: usize,
    },
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Info(String),
    Error(String),
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

    fn config_names(&self) -> Vec<ConfigName> {
        self.entries().iter().map(|e| e.name.clone()).collect()
    }

    fn entry_mut(&mut self, name: &ConfigName) -> Option<&mut ConfigEntry> {
        match &mut self.configs {
            Load::Loaded(entries) => entries.iter_mut().find(|e| &e.name == name),
            _ => None,
        }
    }
}

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
    pub outcome: Result<String, String>,
    pub took: Duration,
    pub at: Option<String>,
    pub msg: Msg,
}

pub fn update(state: &mut State, msg: Msg) -> Vec<Effect> {
    let effects = handle(state, msg);
    state.pending += effects.len();
    effects
}

/// What one message does. The work it starts is counted once, by [`update`], however many messages were
/// unwrapped on the way.
fn handle(state: &mut State, msg: Msg) -> Vec<Effect> {
    match msg {
        Msg::Start => match &state.file {
            Some(view) => vec![
                Effect::LoadFileVersions {
                    config: view.target.config.clone(),
                    path: view.target.path.clone(),
                },
                Effect::LoadHealth,
            ],
            None => vec![Effect::LoadConfigs, Effect::LoadHealth],
        },
        Msg::HealthLoaded(result) => {
            settle(state);
            match result {
                Ok(data) => {
                    state.health.data = Some(*data);
                    state.health.error = None;
                    state.health.keep_cursor_in_range();
                }
                Err(why) => state.health.error = Some(why),
            }
            Vec::new()
        }
        Msg::CleanupSwitched(result) => {
            settle(state);
            match result {
                Ok(()) => {
                    state.notice = Some(Notice::Info("Automatic cleanup is on.".into()));
                    vec![Effect::LoadHealth]
                }
                Err(why) => {
                    state.notice = Some(Notice::Error(format!(
                        "Could not switch automatic cleanup on: {why}"
                    )));
                    Vec::new()
                }
            }
        }
        Msg::Finished(done) => {
            let Finished {
                what,
                outcome,
                took,
                at,
                msg,
            } = *done;
            state.activity.record(Activity {
                at,
                what,
                outcome,
                took,
            });
            // what changed may change how protection stands: look again
            let mut effects = handle(state, msg);
            effects.push(Effect::LoadHealth);
            effects
        }
        Msg::Tick => {
            state.spinner = state.spinner.wrapping_add(1);
            Vec::new()
        }
        Msg::Key(key) => on_key(state, key),
        Msg::PointOfferLoaded { config, id, offer } => {
            settle(state);
            let asked = matches!(
                &state.modal,
                Some(Modal::Restore { config: c, id: i, .. }) if *c == config && *i == id
            );
            if asked {
                state.point_offer = offer.map(|offer| (config, id, offer));
            }
            Vec::new()
        }
        Msg::PointsLoaded(result) => {
            settle(state);
            on_points_loaded(state, result)
        }
        Msg::PointPreviewLoaded(offer) => {
            settle(state);
            if state
                .chosen_point()
                .is_some_and(|p| p.point == offer.point.point)
            {
                state.point_preview = Some(offer);
            }
            Vec::new()
        }
        Msg::PointRestored { point, result } => {
            settle(state);
            on_point_restored(state, &point, result)
        }
        Msg::Protected {
            mount_point,
            result,
        } => {
            settle(state);
            on_protected(state, &mount_point, result)
        }
        Msg::Hover(button) => {
            state.hover = button;
            Vec::new()
        }
        Msg::Click(click) => on_click(state, click),
        Msg::Scroll { pane, delta } => on_scroll(state, pane, delta),
        Msg::ChangesLoaded { config, id, result } => {
            settle(state);
            on_changes_loaded(state, &config, id, result)
        }
        Msg::Restored {
            config,
            id,
            path,
            result,
        } => {
            settle(state);
            on_restored(state, &config, id, path, result)
        }
        Msg::FileVersionsLoaded {
            config,
            path,
            result,
        } => {
            settle(state);
            on_file_versions_loaded(state, &config, &path, result)
        }
        Msg::ConfigsLoaded(result) => {
            settle(state);
            on_configs_loaded(state, result)
        }
        Msg::SnapshotsLoaded { config, result } => {
            settle(state);
            on_snapshots_loaded(state, &config, result)
        }
        Msg::Created(results) => {
            settle(state);
            on_created(state, results)
        }
        Msg::Deleted { config, results } => {
            settle(state);
            on_deleted(state, &config, results)
        }
    }
}

/// One result came back. A result nobody asked for does not push the count below zero.
fn settle(state: &mut State) {
    state.pending = state.pending.saturating_sub(1);
}

/// Switching tabs (1, 2, 3) and the keys of the tab that is showing. `None` is a key the window itself
/// handles, whatever is showing: quit, help, take a snapshot of everything, protect, restore points.
fn on_tab_key(state: &mut State, key: Key) -> Option<Vec<Effect>> {
    if let Key::Char(c) = key {
        if let Some(tab) = Tab::from_key(c) {
            state.tab = tab;
            return Some(Vec::new());
        }
    }
    let own = match state.tab {
        Tab::Snapshots => return None,
        Tab::Health => crate::tabs::health::on_key(state, key),
        Tab::Activity => crate::tabs::activity::on_key(state, key),
    };
    // The snapshot lists are not showing, so the keys that act on them do nothing here.
    own.or_else(|| {
        matches!(
            key,
            Key::Tab | Key::Left | Key::Right | Key::Char('h' | 'l' | 'c' | 'd' | 'u' | ' ')
        )
        .then(Vec::new)
    })
}

fn on_key(state: &mut State, key: Key) -> Vec<Effect> {
    if state.modal.is_some() {
        return on_modal_key(state, key);
    }
    // Moving around and reading stay available while snapper works; starting more work does not.
    if state.pending > 0 && matches!(key, Key::Char('r' | 'c' | 'a' | 'd' | 'u' | 'p' | 'R')) {
        state.notice = Some(Notice::Info(
            "busy: wait for the current operation to finish".into(),
        ));
        return Vec::new();
    }
    if state.file.is_some() {
        return on_file_key(state, key);
    }
    if let Some(effects) = on_tab_key(state, key) {
        return effects;
    }
    match key {
        Key::CtrlC | Key::Char('q') => {
            state.quit = true;
            Vec::new()
        }
        Key::Char('?') => {
            state.modal = Some(Modal::Help { page: 0 });
            Vec::new()
        }
        Key::Char('r') => {
            state.notice = Some(Notice::Info("reloading".into()));
            vec![Effect::LoadConfigs, Effect::LoadHealth]
        }
        Key::Tab => {
            state.focus = match state.focus {
                Pane::Configs => Pane::Snapshots,
                Pane::Snapshots => Pane::Configs,
            };
            Vec::new()
        }
        Key::Left | Key::Char('h') => {
            state.focus = Pane::Configs;
            Vec::new()
        }
        Key::Right | Key::Char('l') => {
            state.focus = Pane::Snapshots;
            Vec::new()
        }
        Key::Up | Key::Char('k') => step(state, -1),
        Key::Down | Key::Char('j') => step(state, 1),
        Key::Home | Key::Char('g') => jump(state, false),
        Key::End | Key::Char('G') => jump(state, true),
        Key::Char(' ') => {
            toggle_mark(state);
            Vec::new()
        }
        Key::Char('c') => open_describe(state, false),
        Key::Char('a') => open_describe(state, true),
        Key::Char('d') => open_delete(state),
        Key::Char('u') => open_restore(state),
        Key::Char('p') => {
            state.modal = Some(Modal::Protect { index: 0 });
            Vec::new()
        }
        Key::Char('R') => {
            state.modal = Some(Modal::Points { index: 0 });
            state.points = None;
            state.point_preview = None;
            vec![Effect::LoadPoints]
        }
        _ => Vec::new(),
    }
}

/// What a prompt becomes after a key, and the work that key starts.
struct Reaction {
    /// The prompt as it now is, or `None` if it closes.
    modal: Option<Modal>,
    effects: Vec<Effect>,
}

impl Reaction {
    /// The prompt stays open, as given (possibly changed by the key).
    fn stays(modal: Modal) -> Self {
        Self {
            modal: Some(modal),
            effects: Vec::new(),
        }
    }

    fn closes() -> Self {
        Self {
            modal: None,
            effects: Vec::new(),
        }
    }

    /// The prompt stays open and work starts (moving to another restore point looks it up).
    fn stays_and_starts(modal: Modal, effects: Vec<Effect>) -> Self {
        Self {
            modal: Some(modal),
            effects,
        }
    }

    /// The prompt is answered: it closes and the work it asked for starts.
    fn closes_and_starts(effects: Vec<Effect>) -> Self {
        Self {
            modal: None,
            effects,
        }
    }
}

fn on_modal_key(state: &mut State, key: Key) -> Vec<Effect> {
    let Some(modal) = state.modal.take() else {
        return Vec::new();
    };
    let reaction = match modal {
        Modal::Help { page } => reading_help(key, page),
        Modal::Protect { index } => protecting(state, key, index),
        Modal::Points { index } => browsing_points(state, key, index),
        Modal::Describe { target, input, .. } => describing(state, key, target, input),
        Modal::ConfirmDelete { config, ids } => confirming_delete(state, key, config, ids),
        Modal::Restore {
            config,
            id,
            path,
            stage,
        } => confirming_restore(state, key, config, id, path, stage),
    };
    state.modal = reaction.modal;
    reaction.effects
}

impl State {
    /// The restore point the points screen has the cursor on.
    pub fn chosen_point(&self) -> Option<&snapall_core::RestorePoint> {
        match &self.modal {
            Some(Modal::Points { index }) => self.points.as_ref()?.get(*index),
            _ => None,
        }
    }
}

/// The restore points arrived: show them, and look up what the first would do.
fn on_points_loaded(
    state: &mut State,
    result: Result<Vec<snapall_core::RestorePoint>, String>,
) -> Vec<Effect> {
    match result {
        Err(why) => {
            state.modal = None;
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
        Ok(points) => {
            let first = points.first().cloned();
            state.points = Some(points);
            first
                .map(|point| vec![Effect::LoadPointPreview { point }])
                .unwrap_or_default()
        }
    }
}

/// The restore points screen: Up and Down choose, Enter or `A` puts every area of the chosen one back,
/// Esc leaves. Nothing happens to a point whose areas already match.
fn browsing_points(state: &mut State, key: Key, index: usize) -> Reaction {
    let count = state.points.as_ref().map_or(0, Vec::len);
    let index = index.min(count.saturating_sub(1));
    let moved = |to: usize, state: &mut State| {
        state.point_preview = None;
        let effects = state
            .points
            .as_ref()
            .and_then(|points| points.get(to))
            .map(|point| {
                vec![Effect::LoadPointPreview {
                    point: point.clone(),
                }]
            })
            .unwrap_or_default();
        Reaction::stays_and_starts(Modal::Points { index: to }, effects)
    };
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Down | Key::Char('j') | Key::Tab if index + 1 < count => moved(index + 1, state),
        Key::Up | Key::Char('k') if index > 0 => moved(index - 1, state),
        Key::Enter | Key::Char('A') => {
            let chosen = state
                .points
                .as_ref()
                .and_then(|points| points.get(index))
                .cloned();
            match (chosen, &state.point_preview) {
                (Some(point), Some(preview))
                    if preview.point.point == point.point && preview.areas_to_change() > 0 =>
                {
                    state.notice = Some(Notice::Info(format!(
                        "putting {} area(s) back to {}",
                        preview.areas_to_change(),
                        point.point
                    )));
                    Reaction::closes_and_starts(vec![Effect::RestorePoint { point }])
                }
                _ => Reaction::stays(Modal::Points { index }),
            }
        }
        _ => Reaction::stays(Modal::Points { index }),
    }
}

/// The offer to protect a part of the disk: Tab and the arrows go to the next one, Enter protects the
/// one shown, Esc leaves. Without administrator rights, or with nothing to offer, it only informs.
fn protecting(state: &mut State, key: Key, index: usize) -> Reaction {
    let count = state.offers.len();
    let index = index.min(count.saturating_sub(1));
    let can_protect = count > 0 && state.context.administrator;
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Enter if !can_protect => Reaction::closes(),
        Key::Enter | Key::Char('y' | 'Y') if can_protect => {
            let offer = &state.offers[index];
            state.notice = Some(Notice::Info(format!(
                "protecting {} ({})",
                offer.title, offer.mount_point
            )));
            let effect = Effect::Protect {
                mount_point: offer.mount_point.clone(),
            };
            Reaction::closes_and_starts(vec![effect])
        }
        Key::Tab | Key::Right | Key::Down | Key::Char('j' | 'l') if count > 1 => {
            Reaction::stays(Modal::Protect {
                index: (index + 1) % count,
            })
        }
        Key::Left | Key::Up | Key::Char('k' | 'h') if count > 1 => {
            Reaction::stays(Modal::Protect {
                index: (index + count - 1) % count,
            })
        }
        _ => Reaction::stays(Modal::Protect { index }),
    }
}

fn on_protected(
    state: &mut State,
    mount_point: &str,
    result: Result<ConfigName, String>,
) -> Vec<Effect> {
    match result {
        Ok(_) => {
            state
                .offers
                .retain(|offer| offer.mount_point != mount_point);
            state.after_reload = Some(Notice::Info(snapall_core::explain::protected_text(
                mount_point,
            )));
            // the new area is in the list from now on, and takes part in "snapshot everything"
            vec![Effect::LoadConfigs]
        }
        Err(why) => {
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
    }
}

/// The help: Enter, Tab or the arrows turn the page, Esc or `q` close it.
fn reading_help(key: Key, page: usize) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Enter | Key::Tab | Key::Right | Key::Left | Key::Char(' ' | 'h' | 'l') => {
            Reaction::stays(Modal::Help {
                page: (page + 1) % HELP_PAGES,
            })
        }
        _ => Reaction::stays(Modal::Help { page }),
    }
}

/// How many pages the help has.
pub const HELP_PAGES: usize = 2;

fn describe(target: Target, input: String, error: Option<String>) -> Modal {
    Modal::Describe {
        target,
        input,
        error,
    }
}

/// The prompt for a snapshot's description: type, Backspace, Enter to create, Esc to cancel.
fn describing(state: &mut State, key: Key, target: Target, mut input: String) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC => Reaction::closes(),
        Key::Enter => create_with(state, target, input),
        Key::Backspace => {
            input.pop();
            Reaction::stays(describe(target, input, None))
        }
        Key::Char(c) if !c.is_control() => {
            if input.chars().count() < Description::MAX_LEN {
                input.push(c);
            }
            Reaction::stays(describe(target, input, None))
        }
        _ => Reaction::stays(describe(target, input, None)),
    }
}

/// Enter in the description prompt: an empty description means the default one, and one that is not
/// valid keeps the prompt open and says why.
fn create_with(state: &mut State, target: Target, input: String) -> Reaction {
    let description = if input.trim().is_empty() {
        Ok(Description::default())
    } else {
        Description::parse(&input)
    };
    match description {
        Ok(description) => {
            let targets = match &target {
                Target::One(config) => vec![config.clone()],
                Target::All => state.config_names(),
            };
            state.notice = Some(Notice::Info(format!(
                "creating {} snapshot(s)",
                targets.len()
            )));
            Reaction::closes_and_starts(vec![Effect::Create {
                targets,
                description,
            }])
        }
        Err(problem) => Reaction::stays(describe(target, input, Some(problem.to_string()))),
    }
}

/// Deleting cannot be undone, so it asks: `y` or Enter confirms, `n` or Esc cancels.
fn confirming_delete(
    state: &mut State,
    key: Key,
    config: ConfigName,
    ids: Vec<SnapshotId>,
) -> Reaction {
    match key {
        Key::Char('y' | 'Y') | Key::Enter => {
            state.notice = Some(Notice::Info(format!("deleting {} snapshot(s)", ids.len())));
            Reaction::closes_and_starts(vec![Effect::Delete { config, ids }])
        }
        Key::Char('n' | 'N') | Key::Esc | Key::CtrlC => Reaction::closes(),
        _ => Reaction::stays(Modal::ConfirmDelete { config, ids }),
    }
}

/// The restore prompt. Only a prompt with real changes to restore can be confirmed, and the system
/// root only when it is a single path of it. Anything else is information, and any key closes it.
fn confirming_restore(
    state: &mut State,
    key: Key,
    config: ConfigName,
    id: SnapshotId,
    path: Option<String>,
    stage: RestoreStage,
) -> Reaction {
    let actionable = matches!(
        &stage,
        RestoreStage::Ready(p) if (path.is_some() || !p.is_system_root) && !p.changes.is_empty()
    );
    // Every area of the snapshot's restore point, when it has one and any of it would change.
    if matches!(key, Key::Char('A')) {
        if let Some((c, i, offer)) = &state.point_offer {
            if *c == config && *i == id && offer.areas_to_change() > 0 {
                let (areas, point) = (offer.areas_to_change(), offer.point.clone());
                state.notice = Some(Notice::Info(format!(
                    "putting {areas} area(s) back to {}",
                    point.point
                )));
                return Reaction::closes_and_starts(vec![Effect::RestorePoint { point }]);
            }
        }
    }
    match key {
        Key::Char('y' | 'Y') | Key::Enter if actionable => {
            state.notice = Some(Notice::Info(match &path {
                Some(path) => format!("restoring {path} from #{id}"),
                None => format!("restoring {config} to #{id}"),
            }));
            Reaction::closes_and_starts(vec![Effect::Restore { config, id, path }])
        }
        Key::Esc | Key::CtrlC | Key::Char('n' | 'N') => Reaction::closes(),
        // information with nothing to confirm: any key closes it (but not while still looking)
        _ if !actionable && stage != RestoreStage::Loading => Reaction::closes(),
        _ => Reaction::stays(Modal::Restore {
            config,
            id,
            path,
            stage,
        }),
    }
}

/// Move the cursor of the focused pane. Changing config loads its snapshots on first visit.
fn step(state: &mut State, delta: isize) -> Vec<Effect> {
    match state.focus {
        Pane::Configs => {
            let len = state.entries().len();
            let next = clamp_step(state.config_cursor, delta, len);
            select_config(state, next)
        }
        Pane::Snapshots => {
            let len = state.displayed_snapshots().len();
            state.snapshot_cursor = clamp_step(state.snapshot_cursor, delta, len);
            Vec::new()
        }
    }
}

fn jump(state: &mut State, to_end: bool) -> Vec<Effect> {
    match state.focus {
        Pane::Configs => {
            let len = state.entries().len();
            let target = if to_end { len.saturating_sub(1) } else { 0 };
            select_config(state, target)
        }
        Pane::Snapshots => {
            let len = state.displayed_snapshots().len();
            state.snapshot_cursor = if to_end { len.saturating_sub(1) } else { 0 };
            Vec::new()
        }
    }
}

fn clamp_step(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    current.saturating_add_signed(delta).min(len - 1)
}

fn select_config(state: &mut State, index: usize) -> Vec<Effect> {
    if index == state.config_cursor || index >= state.entries().len() {
        return Vec::new();
    }
    state.config_cursor = index;
    state.snapshot_cursor = 0;
    state.marked.clear();
    load_selected_if_needed(state)
}

fn load_selected_if_needed(state: &State) -> Vec<Effect> {
    match state.selected_config() {
        Some(entry) if entry.snapshots == Load::NotLoaded => {
            vec![Effect::LoadSnapshots(entry.name.clone())]
        }
        _ => Vec::new(),
    }
}

fn toggle_mark(state: &mut State) {
    if state.focus != Pane::Snapshots {
        return;
    }
    let Some(id) = state.selected_snapshot().map(|s| s.id) else {
        return;
    };
    if !state.marked.remove(&id) {
        state.marked.insert(id);
    }
    // Like a file manager: marking moves on to the next row.
    let len = state.displayed_snapshots().len();
    state.snapshot_cursor = clamp_step(state.snapshot_cursor, 1, len);
}

fn open_describe(state: &mut State, all: bool) -> Vec<Effect> {
    let target = if all {
        if state.entries().is_empty() {
            state.notice = Some(Notice::Error(
                "there are no protected areas to snapshot; press p to protect one".into(),
            ));
            return Vec::new();
        }
        Target::All
    } else {
        match state.selected_config() {
            Some(entry) => Target::One(entry.name.clone()),
            None => {
                state.notice = Some(Notice::Error("no area selected".into()));
                return Vec::new();
            }
        }
    };
    state.modal = Some(Modal::Describe {
        target,
        input: String::new(),
        error: None,
    });
    Vec::new()
}

fn open_delete(state: &mut State) -> Vec<Effect> {
    let Some(config) = state.selected_config().map(|c| c.name.clone()) else {
        return Vec::new();
    };
    if state.focus != Pane::Snapshots {
        state.notice = Some(Notice::Info(
            "move to the snapshots pane (right arrow) to choose what to delete".into(),
        ));
        return Vec::new();
    }
    let ids: Vec<SnapshotId> = if state.marked.is_empty() {
        state
            .selected_snapshot()
            .map(|s| s.id)
            .into_iter()
            .collect()
    } else {
        state.marked.iter().copied().collect()
    };
    if ids.is_empty() {
        state.notice = Some(Notice::Error("no snapshot to delete".into()));
        return Vec::new();
    }
    state.modal = Some(Modal::ConfirmDelete { config, ids });
    Vec::new()
}

fn on_configs_loaded(state: &mut State, result: Result<Vec<ConfigName>, String>) -> Vec<Effect> {
    match result {
        Err(error) => {
            state.configs = Load::Failed(error.clone());
            state.config_cursor = 0;
            state.after_reload = None;
            state.notice = Some(Notice::Error(error));
            Vec::new()
        }
        Ok(mut names) => {
            let previous = state.selected_config().map(|c| c.name.clone());
            names.sort();
            names.dedup();
            state.config_cursor = previous
                .and_then(|p| names.iter().position(|n| *n == p))
                .unwrap_or(0);
            state.configs = Load::Loaded(
                names
                    .into_iter()
                    .map(|name| ConfigEntry {
                        name,
                        snapshots: Load::NotLoaded,
                    })
                    .collect(),
            );
            state.snapshot_cursor = 0;
            state.marked.clear();
            if matches!(state.notice, Some(Notice::Info(_))) {
                state.notice = None;
            }
            if let Some(confirmation) = state.after_reload.take() {
                state.notice = Some(confirmation);
            }
            if state.entries().is_empty() {
                state.notice = Some(Notice::Error(if state.offers.is_empty() {
                    "There are no protected areas yet, and snapall found nothing it can protect here: it works with disks formatted as Btrfs.".into()
                } else {
                    "There are no protected areas yet, so there is nothing to show. Press p to protect one.".into()
                }));
            }
            load_selected_if_needed(state)
        }
    }
}

fn on_snapshots_loaded(
    state: &mut State,
    config: &ConfigName,
    result: Result<Vec<SnapshotInfo>, String>,
) -> Vec<Effect> {
    let selected = state.selected_config().map(|c| c.name.clone());
    let Some(entry) = state.entry_mut(config) else {
        return Vec::new();
    };
    match result {
        Ok(mut list) => {
            list.sort_by_key(|s| s.id);
            entry.snapshots = Load::Loaded(list);
        }
        Err(error) => {
            entry.snapshots = Load::Failed(error.clone());
            state.notice = Some(Notice::Error(format!("{config}: {error}")));
        }
    }
    if selected.as_ref() == Some(config) {
        let existing: BTreeSet<SnapshotId> =
            state.displayed_snapshots().iter().map(|s| s.id).collect();
        state.marked.retain(|id| existing.contains(id));
        let len = existing.len();
        state.snapshot_cursor = state.snapshot_cursor.min(len.saturating_sub(1));
    }
    Vec::new()
}

fn on_created(
    state: &mut State,
    results: Vec<(ConfigName, Result<SnapshotId, String>)>,
) -> Vec<Effect> {
    let created: Vec<String> = results
        .iter()
        .filter_map(|(config, r)| r.as_ref().ok().map(|id| format!("{config} #{id}")))
        .collect();
    let failures: Vec<String> = results
        .iter()
        .filter_map(|(config, r)| r.as_ref().err().map(|e| format!("{config}: {e}")))
        .collect();
    state.notice = Some(if failures.is_empty() {
        Notice::Info(format!("created {}", created.join(", ")))
    } else if created.is_empty() {
        Notice::Error(failures.join("; "))
    } else {
        Notice::Error(format!(
            "created {}, but {}",
            created.join(", "),
            failures.join("; ")
        ))
    });
    // New snapshots are newest: show them.
    state.snapshot_cursor = 0;
    results
        .into_iter()
        .filter_map(|(config, r)| r.ok().map(|_| Effect::LoadSnapshots(config)))
        .collect()
}

fn on_deleted(
    state: &mut State,
    config: &ConfigName,
    results: Vec<(SnapshotId, Result<(), String>)>,
) -> Vec<Effect> {
    let deleted: Vec<String> = results
        .iter()
        .filter(|(_, r)| r.is_ok())
        .map(|(id, _)| format!("#{id}"))
        .collect();
    let failures: Vec<String> = results
        .iter()
        .filter_map(|(id, r)| r.as_ref().err().map(|e| format!("#{id}: {e}")))
        .collect();
    state.notice = Some(if failures.is_empty() {
        Notice::Info(format!("deleted {} from {config}", deleted.join(", ")))
    } else {
        Notice::Error(format!("{config}: {}", failures.join("; ")))
    });
    state.marked.clear();
    vec![Effect::LoadSnapshots(config.clone())]
}

fn open_restore(state: &mut State) -> Vec<Effect> {
    let Some(config) = state.selected_config().map(|c| c.name.clone()) else {
        return Vec::new();
    };
    if state.focus != Pane::Snapshots {
        state.notice = Some(Notice::Info(
            "move to the snapshots pane (right arrow) to choose the snapshot to restore".into(),
        ));
        return Vec::new();
    }
    let Some(id) = state.selected_snapshot().map(|s| s.id) else {
        state.notice = Some(Notice::Error("no snapshot to restore".into()));
        return Vec::new();
    };
    state.modal = Some(Modal::Restore {
        config: config.clone(),
        id,
        path: None,
        stage: RestoreStage::Loading,
    });
    state.point_offer = None;
    vec![
        Effect::LoadChanges {
            config: config.clone(),
            id,
            path: None,
        },
        // whether this snapshot is one of several taken together: the prompt offers them all
        Effect::LoadPointOffer { config, id },
    ]
}

/// A restore point was put back (or part of it failed): say what happened, and how to undo it.
fn on_point_restored(
    state: &mut State,
    point: &str,
    result: Result<PointOutcome, String>,
) -> Vec<Effect> {
    match result {
        Err(why) => {
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
        Ok(outcome) if !outcome.failed.is_empty() => {
            state.notice = Some(Notice::Error(format!(
                "Put {} area(s) back to {point}, but {} failed: {}",
                outcome.restored,
                outcome.failed.len(),
                outcome.failed.join("; ")
            )));
            vec![Effect::LoadConfigs]
        }
        Ok(outcome) => {
            let undo = outcome
                .undo
                .map(|undo| format!(" Undo it: restore point {undo}"))
                .unwrap_or_default();
            state.after_reload = Some(Notice::Info(format!(
                "Put {} area(s) back to {point}.{undo}",
                outcome.restored
            )));
            vec![Effect::LoadConfigs]
        }
    }
}

fn on_changes_loaded(
    state: &mut State,
    config: &ConfigName,
    id: SnapshotId,
    result: Result<Preview, String>,
) -> Vec<Effect> {
    // The answer only matters to the prompt that asked. If it was closed meanwhile, drop it.
    if let Some(Modal::Restore {
        config: asked_config,
        id: asked_id,
        stage,
        ..
    }) = &mut state.modal
    {
        if asked_config == config && *asked_id == id && *stage == RestoreStage::Loading {
            *stage = match result {
                Ok(preview) => RestoreStage::Ready(preview),
                Err(error) => RestoreStage::Failed(error),
            };
        }
    }
    Vec::new()
}

fn on_restored(
    state: &mut State,
    config: &ConfigName,
    id: SnapshotId,
    path: Option<String>,
    result: Result<Restored, String>,
) -> Vec<Effect> {
    state.notice = Some(match (&result, &path) {
        (Ok(done), Some(path)) => Notice::Info(format!(
            "restored {path} from #{id}; the state before is #{}, restore it to undo",
            done.safety_snapshot
        )),
        (Ok(done), None) => Notice::Info(format!(
            "restored {config} to #{id}; the state before is #{}, restore it to undo",
            done.safety_snapshot
        )),
        (Err(error), _) => Notice::Error(error.clone()),
    });
    state.marked.clear();
    // Reload either way: a safety snapshot may exist even when the restore itself failed.
    match &state.file {
        Some(view) => vec![Effect::LoadFileVersions {
            config: view.target.config.clone(),
            path: view.target.path.clone(),
        }],
        None => vec![Effect::LoadSnapshots(config.clone())],
    }
}

// ---- the file history screen --------------------------------------------------------------

fn on_file_key(state: &mut State, key: Key) -> Vec<Effect> {
    let count = state.displayed_versions().len();
    let Some(view) = state.file.as_mut() else {
        return Vec::new();
    };
    match key {
        Key::CtrlC | Key::Char('q') => {
            state.quit = true;
            Vec::new()
        }
        Key::Char('?') => {
            state.modal = Some(Modal::Help { page: 0 });
            Vec::new()
        }
        Key::Up | Key::Char('k') => {
            view.cursor = clamp_step(view.cursor, -1, count);
            Vec::new()
        }
        Key::Down | Key::Char('j') => {
            view.cursor = clamp_step(view.cursor, 1, count);
            Vec::new()
        }
        Key::Home | Key::Char('g') => {
            view.cursor = 0;
            Vec::new()
        }
        Key::End | Key::Char('G') => {
            view.cursor = count.saturating_sub(1);
            Vec::new()
        }
        Key::Char('r') => {
            state.notice = Some(Notice::Info("reloading".into()));
            vec![Effect::LoadFileVersions {
                config: view.target.config.clone(),
                path: view.target.path.clone(),
            }]
        }
        Key::Enter | Key::Char('u') => open_file_restore(state),
        _ => Vec::new(),
    }
}

fn open_file_restore(state: &mut State) -> Vec<Effect> {
    let Some(view) = state.file.as_ref() else {
        return Vec::new();
    };
    let (config, path) = (view.target.config.clone(), view.target.path.clone());
    let cursor = view.cursor;
    let Some(version) = state
        .displayed_versions()
        .get(cursor)
        .map(|v| (v.snapshot.id, v.state))
    else {
        state.notice = Some(Notice::Error("no version to restore".into()));
        return Vec::new();
    };
    let (id, file_state) = version;
    if !file_state.restorable() {
        state.notice = Some(Notice::Info(match file_state {
            FileState::Same => format!("the file is already the same as in #{id}"),
            _ => format!("the file did not exist in #{id}: there is nothing to restore"),
        }));
        return Vec::new();
    }
    state.modal = Some(Modal::Restore {
        config: config.clone(),
        id,
        path: Some(path.clone()),
        stage: RestoreStage::Loading,
    });
    vec![Effect::LoadChanges {
        config,
        id,
        path: Some(path),
    }]
}

fn on_file_versions_loaded(
    state: &mut State,
    config: &ConfigName,
    path: &str,
    result: Result<Vec<FileVersion>, String>,
) -> Vec<Effect> {
    if let Some(view) = state.file.as_mut() {
        if &view.target.config == config && view.target.path == path {
            view.versions = match result {
                Ok(list) => Load::Loaded(list),
                Err(error) => Load::Failed(error),
            };
            let len = match &view.versions {
                Load::Loaded(list) => list.len(),
                _ => 0,
            };
            view.cursor = view.cursor.min(len.saturating_sub(1));
        }
    }
    Vec::new()
}

fn on_click(state: &mut State, click: Click) -> Vec<Effect> {
    match click {
        // A prompt's buttons work only while a prompt is open, and nothing else works then:
        // a click on a button behind a prompt must not be typed into it as a letter.
        Click::Button(
            button @ (Button::Confirm | Button::Cancel | Button::Next | Button::RestoreAll),
        ) => {
            if state.modal.is_some() {
                on_key(state, button.key())
            } else {
                Vec::new()
            }
        }
        _ if state.modal.is_some() => Vec::new(),
        Click::Tab(tab) => {
            state.tab = tab;
            Vec::new()
        }
        Click::Chip(topic) => {
            state.tab = Tab::Health;
            state.health.select_topic(topic);
            Vec::new()
        }
        Click::TabRow { index } => {
            match state.tab {
                Tab::Health if index < state.health.rows().len() => state.health.cursor = index,
                Tab::Activity if index < state.activity.entries.len() => {
                    state.activity.cursor = index;
                }
                _ => {}
            }
            Vec::new()
        }
        Click::Button(button) => on_key(state, button.key()),
        Click::Row { index, .. } if state.file.is_some() => {
            let count = state.displayed_versions().len();
            if let Some(view) = state.file.as_mut() {
                if index < count {
                    view.cursor = index;
                }
            }
            Vec::new()
        }
        Click::Mark { .. } if state.file.is_some() => Vec::new(),
        Click::Row { pane, index } => {
            state.focus = pane;
            match pane {
                Pane::Configs if index < state.entries().len() => select_config(state, index),
                Pane::Snapshots if index < state.displayed_snapshots().len() => {
                    state.snapshot_cursor = index;
                    Vec::new()
                }
                _ => Vec::new(),
            }
        }
        Click::Mark { index } => {
            state.focus = Pane::Snapshots;
            if index < state.displayed_snapshots().len() {
                state.snapshot_cursor = index;
                toggle_mark(state);
                // Marking by click does not move on to the next row, unlike the space key.
                state.snapshot_cursor = index;
            }
            Vec::new()
        }
    }
}

fn on_scroll(state: &mut State, pane: Pane, delta: i32) -> Vec<Effect> {
    if state.modal.is_some() {
        return Vec::new();
    }
    if state.file.is_some() {
        let count = state.displayed_versions().len();
        if let Some(view) = state.file.as_mut() {
            view.cursor = clamp_step(view.cursor, delta as isize, count);
        }
        return Vec::new();
    }
    // The wheel moves the pane under the pointer without taking the keyboard focus from the other.
    let focus = state.focus;
    state.focus = pane;
    let effects = step(state, delta as isize);
    state.focus = focus;
    effects
}
