// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! One folder for each tab, named as the tab bar names it: the snapshots, how protection is doing, and what
//! snapall did. A tab is read in one place: what it does when something arrives, what its keys do, and how it
//! is drawn.

pub mod activity;
pub mod health;
pub mod snapshots;
