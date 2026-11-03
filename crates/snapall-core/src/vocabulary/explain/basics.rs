// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The explanations and cautions that stand on their own: what a snapshot is, what an area is, which kind of disk snapall works with, and what cannot be undone.

/// What snapall can protect: said wherever a person is told that something is out of its reach, so the format is named and
/// explained in one place, in one set of words.
pub const WORKS_WITH: &str = "disks formatted as Btrfs, a disk format that can take snapshots";

/// What a snapshot is, and why it is cheap. Shown first by `snapall setup` and in the TUI's help.
pub const WHAT_IS_A_SNAPSHOT: &str = "A snapshot is a saved moment of your computer. If an update, a new setting or a mistake breaks something, you can go back to how things were.\n\nA snapshot does not copy your files. The disk only remembers what changes afterwards, so a new snapshot takes almost no space, and it grows only as you change things. That is why it is quick, and why it can be kept for a long time.";

/// What an area is, and why it matters which ones are protected.
pub const WHAT_IS_AN_AREA: &str = "Snapshots are kept for one part of your disk at a time, called an area. A snapshot of one area does not contain the others: the System and your Home files are separate areas, so each needs protecting. snapall can only help with an area once it is protected.";

/// What the list of snapshots shows, in one line of the help: the order, and what `before` and `after #3` mean (a pair
/// taken around one command). It has to stay on one row of the help page, which fits a small terminal exactly.
pub const SNAPSHOT_LIST: &str =
    "Newest first. `before` and `after #3` are a pair taken around one command.";

/// What a snapshot is not.
pub const NOT_A_BACKUP: &str = "Snapshots live on the same disk as your files. They protect you from mistakes and bad changes, not from the disk failing or being lost: for that you still need a copy somewhere else.";

/// What to say about a deleted snapshot, which cannot be brought back: what protects against needing to.
pub const DELETE_ADVICE: &str = "A deleted snapshot cannot be brought back. What protects against needing to is a snapshot taken before a risky step: snapall create, or snapall create --group NAME to take one of a whole group.";

/// The same for an area that stopped being protected.
pub const UNPROTECT_ADVICE: &str = "Stopping protection cannot be undone: its snapshots are gone. snapall setup protects the area again, starting fresh with no snapshots. What protects against needing to is a snapshot taken before a risky step: snapall create, or snapall create --group NAME to take one of a whole group.";

/// What a restore puts back besides the files a person is thinking of, and what to do about it. Shown
/// wherever a restore is about to happen.
pub const RESTORE_CAUTION: &str = "Everything in the area goes back, including what programs keep there: their settings, saved sessions, databases and logs. Close the programs that use those files first, and if one file or folder is all you need, restore just that.";

/// Where a restore of the whole computer happens, in two sentences, for the help text.
pub const RESTORING: &str = "To go back, pick a snapshot and choose what to restore: one file, or a whole area such as Home. snapall always takes a snapshot of how things are first, so going back can itself be undone.";
