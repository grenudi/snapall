// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asking the machine what the core wants to know: `df` for a disk, `systemctl` for a background
//! job, `date` for the time, unit files for a job's schedule, `btrfs` for how room is counted, and a lock
//! file for who is making a change. What each program prints is parsed by a small pure function, so it is
//! tested without the program; the asking itself is a few lines and is tested on the machine it runs on.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod asking;
pub mod boot_menu;
pub mod environment;
mod groups;
pub mod installing;
mod journal;
mod lock;
mod machine;
mod quotas;
mod runner;
mod units;

pub use asking::parse_df;
pub use machine::LinuxSystem;
