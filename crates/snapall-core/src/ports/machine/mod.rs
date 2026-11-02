// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the machine can tell snapall besides what snapper says: how full a disk is, whether a
//! background job is switched on and running, what time it is there. A port, like the snapshot
//! backend: the core states what it needs, an adapter answers it, and nothing else knows how.

mod changing;
mod facts;
mod probe;
mod room_counting;
#[cfg(test)]
mod tests;

pub use changing::{Busy, Hold};
pub use facts::{DiskUsage, ServiceState, UnitDetails};
pub use probe::{NoSystem, SystemProbe};
pub use room_counting::{Counting, QuotaMode, Switching};
