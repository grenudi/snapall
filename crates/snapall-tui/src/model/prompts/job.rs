// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The controls of one background job: a key for each thing that can be done to it.

use crate::model::*;
use snapall_core::services::{self, Job, JobAction};
use snapall_core::Act;

/// The key for each action, and for running a timer's work right now.
pub(crate) const KEYS: [(char, JobAction); 5] = [
    ('e', JobAction::EnableNow),
    ('d', JobAction::DisableNow),
    ('s', JobAction::Start),
    ('x', JobAction::Stop),
    ('r', JobAction::Restart),
];

/// The key that runs the work of a timer right now.
pub(crate) const RUN_NOW_KEY: char = 'n';

pub(crate) fn job_of(unit: &str) -> Option<Job> {
    services::ALL.iter().find(|job| job.unit == unit).copied()
}

/// Esc leaves; a letter does what it names, if allowed to; anything else is ignored.
pub(crate) fn controlling_job(state: &mut State, key: Key, unit: String) -> Reaction {
    let Some(job) = job_of(&unit) else {
        return Reaction::closes();
    };
    let chosen = match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => return Reaction::closes(),
        Key::Char(RUN_NOW_KEY) if job.is_timer() => Some((job.worker_unit(), JobAction::Start)),
        Key::Char(c) => KEYS
            .iter()
            .find(|(k, _)| *k == c)
            .map(|(_, action)| (job.unit.to_owned(), *action)),
        _ => None,
    };
    let Some((target, action)) = chosen else {
        return Reaction::stays(Modal::Job { unit });
    };
    if !state.context.administrator {
        state.notice = Some(Notice::Info(
            "Controlling a background job needs administrator rights: start snapall with sudo."
                .into(),
        ));
        return Reaction::stays(Modal::Job { unit });
    }
    let lines = vec![
        format!("{}: {}.", job.title, action.label().to_lowercase()),
        "To put it back, open the job again in the Health tab and choose the opposite.".to_owned(),
    ];
    ask_first(
        Act::ControlJob,
        None,
        "Control a background job",
        lines,
        vec![Effect::ControlJob {
            unit: target,
            action,
        }],
    )
}
