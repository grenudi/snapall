// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A machine whose answers a test sets: its disk, its jobs, its clock, who else is making a change.

mod answering;
mod standing_in;

pub use standing_in::FakeSystem;
