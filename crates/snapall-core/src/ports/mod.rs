// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the core needs from outside, in its own words: a backend for snapshots, the machine, an observer, a way to be asked to stop, and stand-ins for them.

pub mod course;
pub mod machine;
pub mod observing;
pub mod snapshots;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
