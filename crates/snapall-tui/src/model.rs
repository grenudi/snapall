// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pure core of the UI: state, messages, effects and `update`.
//!
//! Nothing here touches a terminal or a backend. `update` mutates the state and returns
//! the *effects* the shell should perform; the shell reports what happened as new messages.

use std::collections::BTreeSet;

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
    Help,
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
pub fn update(state: &mut State, msg: Msg) -> Vec<Effect> {
    let effects = match msg {
        Msg::Start => match &state.file {
            Some(view) => vec![Effect::LoadFileVersions {
                config: view.target.config.clone(),
                path: view.target.path.clone(),
            }],
            None => vec![Effect::LoadConfigs],
        },
        Msg::Tick => {
            state.spinner = state.spinner.wrapping_add(1);
            Vec::new()
        }
        Msg::Key(key) => on_key(state, key),
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
    };
    state.pending += effects.len();
    effects
}

/// One result came back. A result nobody asked for does not push the count below zero.
fn settle(state: &mut State) {
    state.pending = state.pending.saturating_sub(1);
}

fn on_key(state: &mut State, key: Key) -> Vec<Effect> {
    if state.modal.is_some() {
        return on_modal_key(state, key);
    }
    // Moving around and reading stay available while snapper works; starting more work does not.
    if state.pending > 0 && matches!(key, Key::Char('r' | 'c' | 'a' | 'd' | 'u')) {
        state.notice = Some(Notice::Info(
            "busy: wait for the current operation to finish".into(),
        ));
        return Vec::new();
    }
    if state.file.is_some() {
        return on_file_key(state, key);
    }
    match key {
        Key::CtrlC | Key::Char('q') => {
            state.quit = true;
            Vec::new()
        }
        Key::Char('?') => {
            state.modal = Some(Modal::Help);
            Vec::new()
        }
        Key::Char('r') => {
            state.notice = Some(Notice::Info("reloading".into()));
            vec![Effect::LoadConfigs]
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
        _ => Vec::new(),
    }
}

fn on_modal_key(state: &mut State, key: Key) -> Vec<Effect> {
    let Some(modal) = state.modal.take() else {
        return Vec::new();
    };
    match modal {
        Modal::Help => Vec::new(),
        Modal::Describe {
            target,
            mut input,
            error: _,
        } => match key {
            Key::Esc | Key::CtrlC => Vec::new(),
            Key::Backspace => {
                input.pop();
                state.modal = Some(Modal::Describe {
                    target,
                    input,
                    error: None,
                });
                Vec::new()
            }
            Key::Char(c) if !c.is_control() => {
                if input.chars().count() < Description::MAX_LEN {
                    input.push(c);
                }
                state.modal = Some(Modal::Describe {
                    target,
                    input,
                    error: None,
                });
                Vec::new()
            }
            Key::Enter => {
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
                        vec![Effect::Create {
                            targets,
                            description,
                        }]
                    }
                    Err(e) => {
                        state.modal = Some(Modal::Describe {
                            target,
                            input,
                            error: Some(e.to_string()),
                        });
                        Vec::new()
                    }
                }
            }
            _ => {
                state.modal = Some(Modal::Describe {
                    target,
                    input,
                    error: None,
                });
                Vec::new()
            }
        },
        Modal::Restore {
            config,
            id,
            path,
            stage,
        } => {
            // Only a prompt with real changes to restore can be confirmed, and the system root only
            // when it is a single path of it. Anything else is information, and any key closes it.
            let actionable = matches!(
                &stage,
                RestoreStage::Ready(p) if (path.is_some() || !p.is_system_root) && !p.changes.is_empty()
            );
            match key {
                Key::Char('y' | 'Y') | Key::Enter if actionable => {
                    state.notice = Some(Notice::Info(match &path {
                        Some(path) => format!("restoring {path} from #{id}"),
                        None => format!("restoring {config} to #{id}"),
                    }));
                    vec![Effect::Restore { config, id, path }]
                }
                Key::Esc | Key::CtrlC | Key::Char('n' | 'N') => Vec::new(),
                _ if !actionable && stage != RestoreStage::Loading => Vec::new(),
                _ => {
                    state.modal = Some(Modal::Restore {
                        config,
                        id,
                        path,
                        stage,
                    });
                    Vec::new()
                }
            }
        }
        Modal::ConfirmDelete { config, ids } => match key {
            Key::Char('y') | Key::Char('Y') | Key::Enter => {
                state.notice = Some(Notice::Info(format!("deleting {} snapshot(s)", ids.len())));
                vec![Effect::Delete { config, ids }]
            }
            Key::Char('n') | Key::Char('N') | Key::Esc | Key::CtrlC => Vec::new(),
            _ => {
                state.modal = Some(Modal::ConfirmDelete { config, ids });
                Vec::new()
            }
        },
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
            state.notice = Some(Notice::Error("no configs to snapshot".into()));
            return Vec::new();
        }
        Target::All
    } else {
        match state.selected_config() {
            Some(entry) => Target::One(entry.name.clone()),
            None => {
                state.notice = Some(Notice::Error("no config selected".into()));
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
            if state.entries().is_empty() {
                state.notice = Some(Notice::Error(
                    "snapper reports no configs; create one first, for example `snapper -c home create-config /home`"
                        .into(),
                ));
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
    vec![Effect::LoadChanges {
        config,
        id,
        path: None,
    }]
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
            state.modal = Some(Modal::Help);
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
        Click::Button(button @ (Button::Confirm | Button::Cancel)) => {
            if state.modal.is_some() {
                on_key(state, button.key())
            } else {
                Vec::new()
            }
        }
        _ if state.modal.is_some() => Vec::new(),
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
