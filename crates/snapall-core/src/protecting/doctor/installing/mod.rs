// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Understanding the operating system, and how to install the snapshot tool on it.

mod operating_system;
mod plan;

pub use operating_system::{family, Family, OsRelease};
pub use plan::{describe, install_plan, InstallPlan, Step, PACKAGE};

use operating_system::immutable;
