// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! One module per tab that has more to it than the snapshot lists: each owns its state, its keys and
//! its drawing, so a tab is read in one place.

pub mod activity;
pub mod health;
