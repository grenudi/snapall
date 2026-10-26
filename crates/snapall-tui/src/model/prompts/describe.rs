// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The prompt for a snapshot's note: what is typed, and what Enter does with it.

use crate::model::*;

pub(crate) fn describe(target: Target, input: String, error: Option<String>) -> Modal {
    Modal::Describe {
        target,
        input,
        error,
    }
}

/// The prompt for a snapshot's description: type, Backspace, Enter to create, Esc to cancel.
pub(crate) fn describing(
    state: &mut State,
    key: Key,
    target: Target,
    mut input: String,
) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC => Reaction::closes(),
        Key::Enter => create_with(state, target, input),
        Key::Tab => {
            let next = next_target(state, &target);
            Reaction::stays(describe(next, input, None))
        }
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

/// Tab in the prompt for several areas: every protected area, then each group in turn, then every area
/// again. A snapshot of one chosen area has nothing to cycle through.
fn next_target(state: &State, current: &Target) -> Target {
    if matches!(current, Target::One(_)) {
        return current.clone();
    }
    let mut options = vec![Target::All];
    options.extend(state.groups.names().cloned().map(Target::Group));
    let at = options
        .iter()
        .position(|option| option == current)
        .unwrap_or(0);
    options[(at + 1) % options.len()].clone()
}

/// Enter in the description prompt: an empty description means the default one, and one that is not
/// valid keeps the prompt open and says why.
pub(crate) fn create_with(state: &mut State, target: Target, input: String) -> Reaction {
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
                Target::Group(name) => {
                    let wanted = snapall_core::Wanted::Named(name.clone());
                    match state.groups.resolve(&wanted, &state.config_names()) {
                        Ok(areas) => areas,
                        // a group naming an area that is no longer protected is refused, with its name
                        Err(problem) => {
                            return Reaction::stays(describe(
                                target,
                                input,
                                Some(problem.to_string()),
                            ));
                        }
                    }
                }
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

pub(crate) fn open_describe(state: &mut State, all: bool) -> Vec<Effect> {
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
