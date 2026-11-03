// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapall tells a person about what it does, in words that assume nothing: no snapshots, no
//! filesystems, no snapper. Every screen and command that explains something takes its text from
//! here, so they say the same thing, and one test keeps the jargon out of all of it.
//!
//! The vocabulary: a **snapshot** is a saved moment; a **protected area** is a part of the disk that
//! snapshots are kept for (what snapper calls a config, and Btrfs a subvolume: words nobody should
//! need).

mod areas;
mod basics;
mod cleanup;
mod numbers;
#[cfg(test)]
mod tests;

pub use areas::{
    area_detail, area_line, failure_text, protect_offer, protect_summary, protected_text,
};
pub use basics::{
    DELETE_ADVICE, LIMITS_UNKNOWN, NOT_A_BACKUP, NO_RESTORE_POINTS_YET, RESTORE_CAUTION, RESTORING,
    SNAPSHOT_LIST, UNPROTECT_ADVICE, WHAT_IS_AN_AREA, WHAT_IS_A_SNAPSHOT, WORKS_WITH,
};
pub use cleanup::{
    cleanup_from, cleanup_offer, cleanup_text, disks_holding, room_counting_text, seconds_words,
    Cleanup, Limit,
};
pub use numbers::{by_number, counted};
