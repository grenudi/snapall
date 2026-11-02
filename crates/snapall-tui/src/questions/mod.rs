// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The questions the window asks. Each is a folder of its own: what opens it, what its keys do and what comes of the
//! answer (`asking`), and how it is drawn (`drawing`), so a question is read in one place.

pub(crate) mod confirm;
pub(crate) mod delete;
pub(crate) mod describe;
mod frame;
pub(crate) mod groups;
pub(crate) mod help;
pub(crate) mod job;
pub(crate) mod limits;
pub(crate) mod points;
pub(crate) mod protect;
pub(crate) mod putting_back;
pub(crate) mod quotas;
pub(crate) mod restore;
pub(crate) mod stop;
pub(crate) mod unprotect;

/// What every question does: the half that decides, which knows nothing of drawing.
pub(crate) mod asking {
    pub(crate) use super::confirm::asking::*;
    pub(crate) use super::delete::asking::*;
    pub(crate) use super::describe::asking::*;
    pub(crate) use super::groups::asking::*;
    pub(crate) use super::help::asking::*;
    pub(crate) use super::job::asking::*;
    pub(crate) use super::limits::asking::*;
    pub(crate) use super::points::asking::*;
    pub(crate) use super::protect::asking::*;
    pub(crate) use super::quotas::asking::*;
    pub(crate) use super::restore::asking::*;
    pub(crate) use super::stop::asking::*;
    pub(crate) use super::unprotect::asking::*;
}

/// How every question is drawn, in the frame they share.
pub(crate) mod drawing {
    pub(crate) use super::confirm::drawing::*;
    pub(crate) use super::delete::drawing::*;
    pub(crate) use super::describe::drawing::*;
    pub(crate) use super::frame::*;
    pub(crate) use super::groups::drawing::*;
    pub(crate) use super::help::drawing::*;
    pub(crate) use super::job::drawing::*;
    pub(crate) use super::limits::drawing::*;
    pub(crate) use super::points::drawing::*;
    pub(crate) use super::protect::drawing::*;
    pub(crate) use super::quotas::drawing::*;
    pub(crate) use super::restore::drawing::*;
    pub(crate) use super::stop::drawing::*;
    pub(crate) use super::unprotect::drawing::*;
}
