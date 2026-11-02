// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts of a disk that snapshots can protect, and which of them are.
//!
//! A snapshot covers one *subvolume* (snapper calls the thing that sets one up a "area"), and it
//! stops at the boundary of that subvolume. On the usual Arch/CachyOS layout `/` and `/home` are
//! separate subvolumes, so snapshots of `/` contain none of the files under `/home`: that part of the
//! disk needs protecting in its own right. People read this module's words, so they are plain ones:
//! a *protected area* is a part of the disk snapshots are kept for.
//!
//! Nothing here reads the system: the shell hands over the text of `/proc/self/mountinfo` and the
//! subvolume of each area.

mod areas;
mod mounts;
#[cfg(test)]
mod tests;
mod worth;

pub use areas::{areas, gaps, Area};
pub use mounts::{mounts_from_mountinfo, Mount};
pub use worth::{describe, suggest_name, Info, Worth};

use areas::normal;
