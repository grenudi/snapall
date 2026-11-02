// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The words the two ways of putting something back share: how many areas, and that each is saved first.

/// "1 area" or "3 areas".
pub(crate) fn areas_in_words(count: usize) -> String {
    if count == 1 {
        "1 area".to_owned()
    } else {
        format!("{count} areas")
    }
}

/// Said before one area, or a path of it, is put back from a snapshot.
pub(crate) const THE_AREA_IS_SAVED_FIRST: &str =
    "How it is now is saved first, so this can be rolled back: the Activity tab shows how.";

/// Said before every area of a restore point is put back.
pub(crate) const EACH_AREA_IS_SAVED_FIRST: &str =
    "Each area's current state is saved first, so this can be rolled back: the Activity tab shows how.";
