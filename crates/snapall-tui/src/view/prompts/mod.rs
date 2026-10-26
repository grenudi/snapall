// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The prompts: their shared frame, then one module per prompt that says what it shows.

mod delete;
mod describe;
mod frame;
mod groups;
mod help;
mod job;
mod limits;
mod points;
mod protect;
mod quotas;
mod restore;
mod stop;
mod unprotect;

pub(crate) use delete::*;
pub(crate) use describe::*;
pub(crate) use frame::*;
pub(crate) use groups::*;
pub(crate) use help::*;
pub(crate) use job::*;
pub(crate) use limits::*;
pub(crate) use points::*;
pub(crate) use protect::*;
pub(crate) use quotas::*;
pub(crate) use restore::*;
pub(crate) use stop::*;
pub(crate) use unprotect::*;
