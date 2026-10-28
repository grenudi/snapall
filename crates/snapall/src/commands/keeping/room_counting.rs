// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Counting the room that snapshots take.

use super::*;
use crate::ask;
use crate::output::Outcome;
use snapall_core::explain::room_counting_text;
use snapall_core::{gather, Act, Facts, QuotaMode, Switching, SystemProbe};
use std::error::Error;

/// How the disk counts the room snapshots take, and, given `on` or `off`, switching it. Switching changes how the
/// whole disk works, so it explains what it gives and costs and asks first.
pub fn quotas(ctx: &Context, wanted: Option<&str>, yes: bool) -> Outcome {
    let system = snapall_system::LinuxSystem::new();
    let facts = gather(ctx.backend, &system)?;
    let mode = how_the_disk_counts(&facts)?;
    match wanted {
        None => say_how_the_disk_counts(ctx, mode),
        Some(wanted) => switch(ctx, &system, &facts, mode, switch_to(wanted)?, yes),
    }
}

fn switch(
    ctx: &Context,
    system: &dyn SystemProbe,
    facts: &Facts,
    mode: QuotaMode,
    turn_on: bool,
    yes: bool,
) -> Outcome {
    match mode.switching(turn_on) {
        Switching::CannotBeDoneInPlace => Err("the disk counts room the simple way, and snapall cannot switch it to the full kind in place".into()),
        Switching::AlreadyThatWay => say_it_is_already_so(ctx, mode),
        Switching::Possible => {
            let approved = ask_before_switching(facts, mode, turn_on, yes)?;
            make_the_switch(ctx, system, facts, approved, turn_on)
        }
    }
}

fn how_the_disk_counts(facts: &Facts) -> Result<QuotaMode, Box<dyn Error>> {
    let first = facts.areas.first();
    first.map(|area| area.quotas).ok_or_else(|| {
        "nothing is protected yet, so there is nothing to count: run `snapall setup`".into()
    })
}

fn switch_to(wanted: &str) -> Result<bool, Box<dyn Error>> {
    match wanted {
        "on" => Ok(true),
        "off" => Ok(false),
        other => Err(format!("`{other}` is not `on` or `off`").into()),
    }
}

fn say_how_the_disk_counts(ctx: &Context, mode: QuotaMode) -> Outcome {
    let word = mode.word();
    ctx.show.result(
        format!(
            "Counting the room snapshots take: {word}\n\n{}\n",
            room_counting_text(mode)
        ),
        serde_json::json!({ "counting": word, "explanation": room_counting_text(mode) }),
    )
}

fn say_it_is_already_so(ctx: &Context, mode: QuotaMode) -> Outcome {
    let word = mode.word();
    ctx.show.result(
        format!("Counting the room snapshots take is already {word}.\n"),
        serde_json::json!({ "counting": word, "changed": false }),
    )
}

fn ask_before_switching(
    facts: &Facts,
    mode: QuotaMode,
    turn_on: bool,
    yes: bool,
) -> Result<ask::Approved, Box<dyn Error>> {
    let areas: Vec<String> = facts
        .areas
        .iter()
        .map(snapall_core::AreaFacts::title)
        .collect();
    let question = format!(
        "Switch counting {} for the disk{} holding {}?",
        side(turn_on),
        if areas.len() == 1 { "" } else { "s" },
        areas.join(", ")
    );
    ask::before_showing(
        Act::RoomCounting,
        yes,
        || format!("{}\n", room_counting_text(mode)),
        &question,
    )
}

fn make_the_switch(
    ctx: &Context,
    system: &dyn SystemProbe,
    facts: &Facts,
    approved: ask::Approved,
    turn_on: bool,
) -> Outcome {
    let change = Change::begin(
        approved,
        &format!("Switching room counting {}", side(turn_on)),
        &format!("Switched room counting {}", side(turn_on)),
        None,
    )?;
    let result: Result<(), String> = facts
        .areas
        .iter()
        .try_for_each(|area| system.set_quotas(&area.location, turn_on));
    if result.is_ok() {
        change.rollback(snapall_core::rollback::after_room_counting(turn_on));
    }
    change.end_with(&result);
    result?;
    ctx.show.result(
        format!(
            "Counting the room snapshots take is now {}.{}\n",
            side(turn_on),
            if turn_on {
                " The disk builds its tally in the background; figures appear as it finishes."
            } else {
                ""
            }
        ),
        serde_json::json!({ "counting": side(turn_on), "changed": true }),
    )
}

fn side(turn_on: bool) -> &'static str {
    if turn_on {
        "on"
    } else {
        "off"
    }
}
