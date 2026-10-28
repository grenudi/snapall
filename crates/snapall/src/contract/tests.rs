// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contract against its own declarations.

#![cfg(test)]

use super::*;
use snapall_core::{Act, Weight};

/// The commands that carry out an act, from what each command declares it does.
fn commands_doing(act: Act) -> Vec<&'static str> {
    Command::ALL
        .iter()
        .map(|command| command.spec())
        .filter(|spec| spec.acts.contains(&Some(act)))
        .map(|spec| spec.token)
        .collect()
}

/// What the window does and the command line does not, by design: the whole of reading (every command
/// that lists, shows or explains something), and the jobs, which are the system's, not snapall's to script.
const WINDOW_ONLY: [Act; 2] = [Act::Read, Act::ControlJob];

#[test]
fn every_act_is_carried_out_by_a_command_except_what_is_the_windows_alone_and_says_so() {
    for act in Act::EVERY {
        let commands = commands_doing(act);
        if WINDOW_ONLY.contains(&act) {
            assert!(
                commands.is_empty(),
                "{act:?} is declared the window's alone, but `{commands:?}` does it"
            );
        } else {
            assert!(
                !commands.is_empty(),
                "no command carries out {act:?}: the app does something it cannot be told to do"
            );
        }
    }
}

#[test]
fn what_a_command_asks_and_what_a_script_must_say_follow_from_what_it_does() {
    for command in Command::ALL {
        let spec = command.spec();
        let weights: Vec<Weight> = spec.acts.iter().flatten().map(|act| act.weight()).collect();
        assert_eq!(
            spec.destroy,
            weights.contains(&Weight::Destroys),
            "`{}` and --destroy",
            spec.token
        );
        assert_eq!(
            spec.yes,
            weights.contains(&Weight::Changes),
            "`{}` and --yes",
            spec.token
        );
        // --yes cannot answer for a loss: a command that only loses something takes none
        let only_destroys = !weights.is_empty() && weights.iter().all(|w| *w == Weight::Destroys);
        assert!(
            !(only_destroys && spec.yes),
            "`{}` only destroys, so --yes must not be accepted",
            spec.token
        );
    }
}

#[test]
fn every_command_has_its_own_words_and_the_derived_lists_say_what_the_declarations_say() {
    let mut seen = std::collections::BTreeSet::new();
    for command in Command::ALL {
        let spec = command.spec();
        assert!(
            seen.insert(spec.token),
            "`{}` is declared twice",
            spec.token
        );
        assert!(
            !spec.summary.is_empty() && spec.summary.chars().next().is_some_and(char::is_uppercase),
            "{}: a summary is a sentence",
            spec.token
        );
        assert_eq!(Command::from_token(spec.token), Some(*command));
        assert_eq!(JSON_COMMANDS.contains(command), spec.json, "{}", spec.token);
        assert_eq!(
            LOGGED_COMMANDS.contains(command),
            spec.logged,
            "{}",
            spec.token
        );
        assert_eq!(YES_COMMANDS.contains(command), spec.yes, "{}", spec.token);
        assert_eq!(
            DESTROY_COMMANDS.contains(command),
            spec.destroy,
            "{}",
            spec.token
        );
        assert_eq!(
            DRY_RUN_COMMANDS.contains(command),
            spec.dry_run,
            "{}",
            spec.token
        );
    }
}
