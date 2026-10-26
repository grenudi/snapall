// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! One module per prompt: what opens it, what its keys do, and what comes of the answer.

mod delete;
mod describe;
mod help;
mod points;
mod protect;
mod restore;

pub(crate) use delete::*;
pub(crate) use describe::*;
pub(crate) use help::*;
pub(crate) use points::*;
pub(crate) use protect::*;
pub(crate) use restore::*;
