// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What moment it is, as the number that names a restore point.

use snapall_core::{fresh_point, Point, SnapshotBackend};

/// A restore point named after now: every snapshot of one action is the same point.
pub(super) fn a_restore_point_for_now(backend: &dyn SnapshotBackend) -> Point {
    fresh_point(backend, unix_now())
}

/// Seconds since 1970, which is what names a restore point after the moment an action started.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
