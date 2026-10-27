// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Counting the room that snapshots take.

use super::*;
use crate::ask;
use crate::output::Outcome;
use snapall_core::{gather, Act};

/// How the disk counts the room snapshots take, and, given `on` or `off`, switching it. Switching changes
/// how the whole disk works, so it explains what it gives and costs and asks first.
pub fn quotas(ctx: &Context, wanted: Option<&str>, yes: bool) -> Outcome {
    use snapall_core::explain::room_counting_text;
    use snapall_core::{QuotaMode, SystemProbe};
    let system = snapall_system::LinuxSystem::new();
    let facts = gather(ctx.backend, &system)?;
    let Some(first) = facts.areas.first() else {
        return Err(
            "nothing is protected yet, so there is nothing to count: run `snapall setup`".into(),
        );
    };
    let mode = first.quotas;
    let word = match mode {
        QuotaMode::Off => "off",
        QuotaMode::Full => "on",
        QuotaMode::Simple => "on, the simple kind",
        QuotaMode::Unknown => "could not be found out",
    };
    let Some(wanted) = wanted else {
        return ctx.show.result(
            format!(
                "Counting the room snapshots take: {word}\n\n{}\n",
                room_counting_text(mode)
            ),
            serde_json::json!({ "counting": word, "explanation": room_counting_text(mode) }),
        );
    };
    let turn_on = match wanted {
        "on" => true,
        "off" => false,
        other => return Err(format!("`{other}` is not `on` or `off`").into()),
    };
    if mode == QuotaMode::Simple && turn_on {
        return Err("the disk counts room the simple way, and snapall cannot switch it to the full kind in place".into());
    }
    if (turn_on && mode == QuotaMode::Full) || (!turn_on && mode == QuotaMode::Off) {
        return ctx.show.result(
            format!("Counting the room snapshots take is already {word}.\n"),
            serde_json::json!({ "counting": word, "changed": false }),
        );
    }
    let areas: Vec<String> = facts
        .areas
        .iter()
        .map(snapall_core::AreaFacts::title)
        .collect();
    if !yes && ask::can_ask() {
        eprintln!("{}\n", room_counting_text(mode));
    }
    let question = format!(
        "Switch counting {} for the disk{} holding {}?",
        if turn_on { "on" } else { "off" },
        if areas.len() == 1 { "" } else { "s" },
        areas.join(", ")
    );
    ask::gate(Act::RoomCounting.weight(), yes, &question, None)?;
    let side = if turn_on { "on" } else { "off" };
    let change = Change::begin(
        &format!("Switching room counting {side}"),
        &format!("Switched room counting {side}"),
        None,
    )?;
    let result: Result<(), String> = facts
        .areas
        .iter()
        .try_for_each(|area| system.set_quotas(&area.subvolume, turn_on));
    if result.is_ok() {
        change.rollback(snapall_core::rollback::after_room_counting(turn_on));
    }
    change.end(result.clone().map(|()| String::new()));
    result?;
    ctx.show.result(
        format!(
            "Counting the room snapshots take is now {}.{}\n",
            if turn_on { "on" } else { "off" },
            if turn_on {
                " The disk builds its tally in the background; figures appear as it finishes."
            } else {
                ""
            }
        ),
        serde_json::json!({ "counting": if turn_on { "on" } else { "off" }, "changed": true }),
    )
}
