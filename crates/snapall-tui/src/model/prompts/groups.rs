// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The groups of areas: see them, make one, change which areas one has, take one away. Three screens, each
//! a small function of a key: the list, naming a new group, and choosing its areas.

use crate::model::*;
use snapall_core::Act;
use snapall_core::{GroupName, Groups};

fn list(cursor: usize) -> Modal {
    Modal::Groups(GroupsScreen::List { cursor })
}

/// The rows of the list: `all`, then the groups made, by name.
pub(crate) fn rows(groups: &Groups) -> Vec<Option<GroupName>> {
    std::iter::once(None)
        .chain(groups.names().cloned().map(Some))
        .collect()
}

/// Saving writes a file only an administrator may write: say so rather than fail.
fn needs_rights(state: &mut State) -> bool {
    if state.context.administrator {
        return false;
    }
    state.notice = Some(Notice::Info(
        "Changing the groups needs administrator rights: start snapall with sudo.".into(),
    ));
    true
}

pub(crate) fn managing_groups(state: &mut State, key: Key, screen: GroupsScreen) -> Reaction {
    match screen {
        GroupsScreen::List { cursor } => on_list(state, key, cursor),
        GroupsScreen::Name { input, error } => on_name(state, key, input, error),
        GroupsScreen::Pick {
            name,
            picks,
            cursor,
            error,
        } => on_pick(state, key, name, picks, cursor, error),
    }
}

fn on_list(state: &mut State, key: Key, cursor: usize) -> Reaction {
    let rows = rows(&state.groups);
    let last = rows.len().saturating_sub(1);
    let broken = state.groups_error.is_some();
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Up | Key::Char('k') => Reaction::stays(list(cursor.saturating_sub(1))),
        Key::Down | Key::Char('j') => Reaction::stays(list((cursor + 1).min(last))),
        // a file that could not be read is not overwritten: whatever was in it may be worth keeping
        Key::Char('n' | 'd') | Key::Enter if broken => {
            state.notice = Some(Notice::Error(
                "The groups file could not be read, so it is left alone: fix it first (the message says where)".into(),
            ));
            Reaction::stays(list(cursor))
        }
        Key::Char('n') => Reaction::stays(Modal::Groups(GroupsScreen::Name {
            input: String::new(),
            error: None,
        })),
        Key::Enter => match rows.get(cursor).cloned().flatten() {
            None => {
                state.notice = Some(Notice::Info(
                    "all is every protected area: it is always there and cannot be changed".into(),
                ));
                Reaction::stays(list(cursor))
            }
            Some(name) => Reaction::stays(pick(state, name)),
        },
        Key::Char('d') => match rows.get(cursor).cloned().flatten() {
            None => {
                state.notice = Some(Notice::Info("all cannot be taken away".into()));
                Reaction::stays(list(cursor))
            }
            Some(_) if needs_rights(state) => Reaction::stays(list(cursor)),
            Some(name) => {
                let before = state.groups.clone();
                let mut groups = before.clone();
                groups.remove(&name);
                let lines = vec![
                    format!("Take the group {name} away?"),
                    "The areas and their snapshots are untouched.".to_owned(),
                ];
                ask_first_then_return(
                    Act::ChangeGroups,
                    lines,
                    vec![Effect::SaveGroups { groups, before }],
                    list(cursor.saturating_sub(1)),
                )
            }
        },
        _ => Reaction::stays(list(cursor)),
    }
}

/// The areas to choose from, with those the group has already ticked.
fn pick(state: &State, name: GroupName) -> Modal {
    let already = state.groups.get(&name).unwrap_or(&[]).to_vec();
    let picks = state
        .config_names()
        .into_iter()
        .map(|area| {
            let ticked = already.contains(&area);
            (area, ticked)
        })
        .collect();
    Modal::Groups(GroupsScreen::Pick {
        name,
        picks,
        cursor: 0,
        error: None,
    })
}

fn on_name(state: &mut State, key: Key, mut input: String, error: Option<String>) -> Reaction {
    let name =
        |input: String, error: Option<String>| Modal::Groups(GroupsScreen::Name { input, error });
    match key {
        Key::Esc | Key::CtrlC => Reaction::stays(list(0)),
        Key::Backspace => {
            input.pop();
            Reaction::stays(name(input, None))
        }
        Key::Char(c) if !c.is_control() => {
            if input.chars().count() < GroupName::MAX_LEN {
                input.push(c);
            }
            Reaction::stays(name(input, None))
        }
        Key::Enter => match GroupName::parse(&input) {
            Ok(parsed) => Reaction::stays(pick(state, parsed)),
            Err(problem) => Reaction::stays(name(input, Some(problem.to_string()))),
        },
        _ => Reaction::stays(name(input, error)),
    }
}

fn on_pick(
    state: &mut State,
    key: Key,
    name: GroupName,
    mut picks: Vec<(ConfigName, bool)>,
    cursor: usize,
    error: Option<String>,
) -> Reaction {
    let last = picks.len().saturating_sub(1);
    let screen =
        |name: GroupName, picks: Vec<(ConfigName, bool)>, cursor: usize, error: Option<String>| {
            Modal::Groups(GroupsScreen::Pick {
                name,
                picks,
                cursor,
                error,
            })
        };
    match key {
        Key::Esc | Key::CtrlC => Reaction::stays(list(0)),
        Key::Up | Key::Char('k') => {
            Reaction::stays(screen(name, picks, cursor.saturating_sub(1), error))
        }
        Key::Down | Key::Char('j') => {
            Reaction::stays(screen(name, picks, (cursor + 1).min(last), error))
        }
        Key::Char(' ') => {
            if let Some(pick) = picks.get_mut(cursor) {
                pick.1 = !pick.1;
            }
            Reaction::stays(screen(name, picks, cursor, None))
        }
        Key::Enter => {
            let chosen: Vec<ConfigName> =
                picks.iter().filter(|p| p.1).map(|p| p.0.clone()).collect();
            if chosen.is_empty() {
                let why = "Choose at least one area (Space includes or leaves out the one under the cursor)".to_owned();
                return Reaction::stays(screen(name, picks, cursor, Some(why)));
            }
            if needs_rights(state) {
                return Reaction::stays(screen(name, picks, cursor, None));
            }
            let areas_text: Vec<String> = chosen.iter().map(ToString::to_string).collect();
            let before = state.groups.clone();
            let mut groups = before.clone();
            if let Err(problem) = groups.set(name.clone(), chosen) {
                return Reaction::stays(screen(name, picks, cursor, Some(problem.to_string())));
            }
            let at = rows(&groups)
                .iter()
                .position(|row| row.as_ref() == Some(&name))
                .unwrap_or(0);
            let lines = vec![
                format!("Make the group {name} of {}.", areas_text.join(", ")),
                "A group only chooses areas to snapshot together; the areas and their snapshots are untouched.".to_owned(),
            ];
            ask_first_then_return(
                Act::ChangeGroups,
                lines,
                vec![Effect::SaveGroups { groups, before }],
                list(at),
            )
        }
        _ => Reaction::stays(screen(name, picks, cursor, error)),
    }
}
