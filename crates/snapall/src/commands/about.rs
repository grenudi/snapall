// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The program speaking about itself: its help, its version and its contract.

use super::*;
use crate::contract::{Contract, VERSION};
use crate::output::Outcome;

pub(super) fn help(ctx: &Context) -> Outcome {
    ctx.show
        .text(&crate::help::render(&Contract::records(), VERSION))
}

pub(super) fn version(ctx: &Context) -> Outcome {
    ctx.show.text(&format!("snapall {VERSION}\n"))
}

pub(super) fn contract(ctx: &Context) -> Outcome {
    ctx.show.json(&argenv_resolve::document(
        &format!("snapall@{VERSION}"),
        &Contract::records(),
    ))
}
