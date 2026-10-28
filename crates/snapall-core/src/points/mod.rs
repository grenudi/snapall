// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restore points: the snapshots that one action took together, one in every area, found again and
//! put back with a single action.
//!
//! A point is found by its tag. Snapshots from before the tag existed are matched instead: the same
//! description in different areas, taken within two minutes of each other, which is exactly what one
//! `snapall` did. A matched point is marked as a guess, so nobody mistakes it for a fact.
//!
//! The system itself is never put back while it runs (see ADR 12): restoring a point puts back every
//! other area, and says so plainly about the system.

mod finding;
mod putting_back;
#[cfg(test)]
mod tests;

pub use finding::{fresh_point, point_of, restore_points, Member, RestorePoint};
pub use putting_back::{
    preview_point, restore_point, restore_point_along, MemberPreview, MemberReport, MemberResult,
    MemberView, PointRestore,
};
