// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Activity tab: what snapall did, newest first, and how each went. It shows the journal, so it
//! reaches back past this window to earlier sessions and shows what the commands did too; a failure
//! stays visible with its reason long after the one-line message has gone, and a change whose program
//! was closed or killed is shown as stopped instead of vanishing.
//!
//! Where nothing is remembered (a machine with no journal) it shows what this window did, so the tab
//! is never empty of what just happened.

mod drawing;
mod entries;
mod keys;

pub use drawing::{render, row_at};
pub use entries::{Activity, ActivityTab};
pub use keys::on_key;
