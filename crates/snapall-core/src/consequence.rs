// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What an action weighs, and so what has to be typed before it happens.
//!
//! Three weights, because a confirmation only protects when it is rare enough to be read:
//!
//! * **Light**: reading, and taking a snapshot. Nothing is asked. A snapshot is the safety net itself: if making
//!   the net took a ceremony, people would skip it, and a prompt on everything teaches people to type `y`
//!   without reading, which weakens the prompts that matter.
//! * **Changes**: something is altered that can be put back, but putting it back takes time and attention.
//!   `y` or `yes` is typed (never just Enter, which is what a hand does by itself).
//! * **Destroys**: something is lost for good. The word `destroy` is typed, and where there is one thing it is
//!   done to, its name with it (`destroy home`): the word says "I understand", the name says "and it is this
//!   one", which guards against the wrong row.
//!
//! One table says which action is which, so that the command line, the window and the tests cannot disagree.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Light,
    Changes,
    Destroys,
}

impl Weight {
    /// The word for it, as it is written in the journal and the JSON.
    pub fn word(self) -> &'static str {
        match self {
            Weight::Light => "light",
            Weight::Changes => "changes",
            Weight::Destroys => "destroys",
        }
    }

    /// [`Weight::word`] the other way.
    pub fn from_word(word: &str) -> Option<Weight> {
        [Weight::Light, Weight::Changes, Weight::Destroys]
            .into_iter()
            .find(|weight| weight.word() == word)
    }

    /// Whether anything is asked.
    pub fn asks(self) -> bool {
        self != Weight::Light
    }

    /// Whether what was typed is the answer. Only the ends are trimmed: `destroy  home` is not `destroy home`.
    pub fn accepts(self, typed: &str, target: Option<&str>) -> bool {
        let typed = typed.trim();
        match self {
            Weight::Light => true,
            Weight::Changes => matches!(typed.to_ascii_lowercase().as_str(), "y" | "yes"),
            Weight::Destroys => match target {
                Some(name) => typed == format!("destroy {name}"),
                None => typed == "destroy",
            },
        }
    }

    /// What to say about what to type.
    pub fn instruction(self, target: Option<&str>) -> String {
        match (self, target) {
            (Weight::Light, _) => String::new(),
            (Weight::Changes, _) => "Type y or yes, then Enter, to go ahead".to_owned(),
            (Weight::Destroys, Some(name)) => {
                format!("Type destroy {name}, then Enter, to go ahead")
            }
            (Weight::Destroys, None) => "Type destroy, then Enter, to go ahead".to_owned(),
        }
    }

    /// The flag that stands for the typed answer where there is nobody to type it (a script). The weight of
    /// the flag is the weight of the thing: `--yes` cannot answer a question that needs `destroy`.
    pub fn script_flag(self) -> Option<&'static str> {
        match self {
            Weight::Light => None,
            Weight::Changes => Some("--yes"),
            Weight::Destroys => Some("--destroy"),
        }
    }
}

/// Everything snapall does, as far as asking goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Read,
    TakeSnapshot,
    DeleteSnapshots,
    /// Put one area back to a snapshot (how it is now is saved first).
    Restore,
    /// Put every area of a restore point back (each is saved first).
    RestorePoint,
    /// Start protecting a part of the disk.
    Protect,
    /// Stop protecting an area: its snapshots are deleted.
    StopProtecting,
    /// Change how many an area keeps; `removes` if the next cleanup would delete snapshots by it.
    ChangeLimits {
        removes: bool,
    },
    /// Switch counting of the room snapshots take on or off.
    RoomCounting,
    ChangeGroups,
    /// Start, stop, enable or disable a background job.
    ControlJob,
    /// Ask a change that is running to stop.
    StopChange,
    InstallSnapper,
    /// Roll something back: it weighs what the rolling back does.
    Undo {
        doing: Weight,
    },
}

impl Act {
    pub fn weight(self) -> Weight {
        match self {
            Act::Read | Act::TakeSnapshot => Weight::Light,
            Act::DeleteSnapshots | Act::StopProtecting | Act::ChangeLimits { removes: true } => {
                Weight::Destroys
            }
            Act::Restore
            | Act::RestorePoint
            | Act::Protect
            | Act::ChangeLimits { removes: false }
            | Act::RoomCounting
            | Act::ChangeGroups
            | Act::ControlJob
            | Act::StopChange
            | Act::InstallSnapper => Weight::Changes,
            Act::Undo { doing } => doing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_for_a_weight_round_trip() {
        for weight in [Weight::Light, Weight::Changes, Weight::Destroys] {
            assert_eq!(Weight::from_word(weight.word()), Some(weight));
        }
        assert_eq!(Weight::from_word("enormous"), None);
    }

    #[test]
    fn reading_and_taking_a_snapshot_never_ask() {
        for act in [Act::Read, Act::TakeSnapshot] {
            assert!(!act.weight().asks(), "{act:?}");
        }
        assert_eq!(Weight::Light.script_flag(), None);
    }

    #[test]
    fn what_loses_something_for_good_needs_destroy_and_everything_else_that_changes_needs_yes() {
        for act in [
            Act::DeleteSnapshots,
            Act::StopProtecting,
            Act::ChangeLimits { removes: true },
        ] {
            assert_eq!(act.weight(), Weight::Destroys, "{act:?}");
        }
        for act in [
            Act::Restore,
            Act::RestorePoint,
            Act::Protect,
            Act::ChangeLimits { removes: false },
            Act::RoomCounting,
            Act::ChangeGroups,
            Act::ControlJob,
            Act::StopChange,
            Act::InstallSnapper,
        ] {
            assert_eq!(act.weight(), Weight::Changes, "{act:?}");
        }
    }

    #[test]
    fn rolling_something_back_weighs_what_the_rolling_back_does() {
        assert_eq!(
            Act::Undo {
                doing: Weight::Changes
            }
            .weight(),
            Weight::Changes
        );
        assert_eq!(
            Act::Undo {
                doing: Weight::Destroys
            }
            .weight(),
            Weight::Destroys,
            "undoing a snapshot is deleting it"
        );
    }

    #[test]
    fn a_change_is_answered_by_y_or_yes_in_any_case_and_by_nothing_else() {
        for yes in ["y", "Y", "yes", "YES", "Yes", "  yes  ", "y\n"] {
            assert!(Weight::Changes.accepts(yes, None), "{yes:?}");
        }
        for no in [
            "",
            " ",
            "n",
            "no",
            "ye",
            "yep",
            "yes please",
            "yy",
            "destroy",
            "ok",
            "1",
            "true",
        ] {
            assert!(!Weight::Changes.accepts(no, None), "{no:?}");
        }
    }

    #[test]
    fn destroying_is_answered_by_the_word_and_the_name_exactly_and_y_is_never_enough() {
        assert!(Weight::Destroys.accepts("destroy home", Some("home")));
        assert!(
            Weight::Destroys.accepts("  destroy home\n", Some("home")),
            "the ends are trimmed"
        );
        for no in [
            "",
            "y",
            "yes",
            "destroy",
            "home",
            "Destroy home",
            "DESTROY home",
            "destroy HOME",
            "destroy  home",
            "destroy home!",
            "destroy data",
            "destroyhome",
        ] {
            assert!(!Weight::Destroys.accepts(no, Some("home")), "{no:?}");
        }
        // with nothing in particular it is done to, the word alone
        assert!(Weight::Destroys.accepts("destroy", None));
        assert!(
            !Weight::Destroys.accepts("yes", None) && !Weight::Destroys.accepts("Destroy", None)
        );
    }

    #[test]
    fn what_to_type_is_said_in_words_and_a_script_has_a_flag_of_the_same_weight() {
        assert_eq!(
            Weight::Changes.instruction(None),
            "Type y or yes, then Enter, to go ahead"
        );
        assert_eq!(
            Weight::Destroys.instruction(Some("home")),
            "Type destroy home, then Enter, to go ahead"
        );
        assert_eq!(
            Weight::Destroys.instruction(None),
            "Type destroy, then Enter, to go ahead"
        );
        assert_eq!(
            (
                Weight::Changes.script_flag(),
                Weight::Destroys.script_flag()
            ),
            (Some("--yes"), Some("--destroy"))
        );
    }
}
