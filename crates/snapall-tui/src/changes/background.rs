// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A change handed to the background: the command that carries it out, and the name it is given.

use crate::model::Effect;

/// The command line that does a change on its own, for the changes slow enough, and made of enough
/// steps, to be worth handing to a process that goes on without the window: a snapshot of several areas,
/// and a restore point put back area by area. A change of one area takes a moment and stays here.
pub(crate) fn background_arguments(effect: &Effect) -> Option<Vec<String>> {
    match effect {
        Effect::Create {
            targets,
            description,
        } if targets.len() > 1 => Some(vec![
            "create".to_owned(),
            "--only".to_owned(),
            targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            // free text: after the `--`, so that a note which begins with a dash stays a note
            "--".to_owned(),
            description.to_string(),
        ]),
        Effect::RestorePoint { point } => Some(vec![
            "restore-all".to_owned(),
            point.point.to_string(),
            "--yes".to_owned(),
        ]),
        _ => None,
    }
}

/// A name for one change that no other is likely to have: the program making it, when, and which of its
/// changes it is.
pub(crate) fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let milliseconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    format!(
        "{}-{milliseconds}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed)
    )
}
