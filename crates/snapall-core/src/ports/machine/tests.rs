// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the machine can be asked.

#![cfg(test)]

use super::*;

#[test]
fn switching_room_counting_is_already_so_impossible_in_place_or_possible() {
    use QuotaMode::{Full, Off, Simple, Unknown};
    assert_eq!(Full.switching(Counting::On), Switching::AlreadyThatWay);
    assert_eq!(Off.switching(Counting::Off), Switching::AlreadyThatWay);
    assert_eq!(
        Simple.switching(Counting::On),
        Switching::CannotBeDoneInPlace
    );
    for (mode, on) in [
        (Off, Counting::On),
        (Full, Counting::Off),
        (Simple, Counting::Off),
        (Unknown, Counting::On),
        (Unknown, Counting::Off),
    ] {
        assert_eq!(
            mode.switching(on),
            Switching::Possible,
            "{mode:?} to {on:?}"
        );
    }
}
