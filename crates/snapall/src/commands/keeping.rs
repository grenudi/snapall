// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What is kept: how many snapshots an area holds, whether the room they take is counted, and the groups of areas.

use super::*;
use crate::output::Outcome;
use crate::{ask, render};
use snapall_core::{gather, Act, ConfigName};
use std::error::Error;

/// What was asked for, as typed: each may be absent.
pub struct LimitsWanted<'a> {
    pub keep: Option<&'a str>,
    pub min_age: Option<&'a str>,
    pub cleanup: Option<&'a str>,
}

/// How many snapshots an area keeps, and when its oldest go. Without options it shows them and what the
/// next cleanup would remove; with options it changes them, after saying what that would remove (a removed
/// snapshot cannot be brought back, so a change that removes any asks first, or needs `--yes`).
pub fn limits(
    ctx: &Context,
    area: &str,
    wanted: LimitsWanted<'_>,
    yes: bool,
    destroy: bool,
) -> Outcome {
    use snapall_core::{limits_words, preview_limits, Limits, SystemProbe};
    let config = ConfigName::parse(area)?;
    if !ctx.backend.list_configs()?.contains(&config) {
        return Err(format!("`{area}` is not a protected area: `snapall list` shows them").into());
    }
    let before = Limits::from_retention(&ctx.backend.retention(&config)?).ok_or(
        "snapper did not say how many this area keeps, so there is nothing to show or change",
    )?;
    let number = |text: &str, what: &str| -> Result<u64, Box<dyn Error>> {
        text.trim()
            .parse::<u64>()
            .map_err(|_| format!("{what} must be a whole number, not `{text}`").into())
    };
    let mut after = before;
    if let Some(text) = wanted.keep {
        after.keep = u32::try_from(number(text, "--keep")?).map_err(|_| "--keep is too large")?;
    }
    if let Some(text) = wanted.min_age {
        after.min_age_seconds = number(text, "--min-age")? * 60;
    }
    match wanted.cleanup {
        Some("on") => after.cleanup = true,
        Some("off") => after.cleanup = false,
        Some(other) => return Err(format!("--cleanup must be `on` or `off`, not `{other}`").into()),
        None => {}
    }
    if let Some(problem) = after.problem() {
        return Err(problem.into());
    }
    let system = snapall_system::LinuxSystem::new();
    let now = system
        .local_time()
        .as_deref()
        .and_then(snapall_core::calendar::seconds_from_snapper_date);
    let snapshots = ctx.backend.list_snapshots(&config)?;
    let preview = preview_limits(&snapshots, &after, now);
    let describe = |l: &Limits| {
        format!(
            "keeps the newest {}, never removes one younger than {} minutes, cleanup {}",
            l.keep,
            l.min_age_seconds / 60,
            if l.cleanup { "on" } else { "off" }
        )
    };
    let options = after.changes_from(&before);
    if options.is_empty() {
        return ctx.show.result(
            format!("{config} {}.
{}
", describe(&before), limits_words(&preview, &before)),
            serde_json::json!({ "area": area, "keep": before.keep, "min_age_minutes": before.min_age_seconds / 60, "cleanup": before.cleanup, "would_remove": preview.would_remove, "changed": false }),
        );
    }
    eprintln!(
        "{config} currently {}.
After: {}.
{}
",
        describe(&before),
        describe(&after),
        limits_words(&preview, &after)
    );
    // a change that would remove snapshots is destroying; any other is a change like the rest
    let removes = preview.would_remove > 0;
    let act = Act::ChangeLimits { removes };
    if let Some(stop) = ask::gate(
        act.weight(),
        if removes { destroy } else { yes },
        "Apply this?",
        None,
    )? {
        return Ok(stop);
    }
    let change = Change::begin(
        &format!("Changing how many snapshots {config} keeps"),
        &format!("Changed how many snapshots {config} keeps"),
        None,
    )?;
    let result: Result<(), String> = options.iter().try_for_each(|option| {
        ctx.backend
            .set_config_option(&config, *option)
            .map_err(|e| e.to_string())
    });
    if result.is_ok() {
        change.rollback(snapall_core::rollback::after_limits(
            &config.to_string(),
            &before,
        ));
    }
    change.end(result.clone().map(|()| String::new()));
    result?;
    ctx.show.result(
        format!("{config} now {}. It takes effect at the next cleanup.
", describe(&after)),
        serde_json::json!({ "area": area, "keep": after.keep, "min_age_minutes": after.min_age_seconds / 60, "cleanup": after.cleanup, "would_remove": preview.would_remove, "changed": true }),
    )
}

/// The areas of a group, out of the ones protected now; a group that names one that is not is refused.
pub(super) fn areas_of_group(
    ctx: &Context,
    wanted: &str,
) -> Result<Vec<ConfigName>, Box<dyn Error>> {
    use snapall_core::{SystemProbe, Wanted};
    let wanted = Wanted::parse(wanted)?;
    let groups = snapall_system::LinuxSystem::new().groups()?;
    let protected = ctx.backend.list_configs()?;
    Ok(groups.resolve(&wanted, &protected)?)
}

/// Sets of areas that have a name. Without an action they are listed; `add` makes or replaces one and
/// `remove` takes one away. `all` is always there, is every protected area, and cannot be changed.
pub fn groups(
    ctx: &Context,
    action: Option<&str>,
    name: Option<&str>,
    areas: Option<&str>,
    yes: bool,
) -> Outcome {
    use snapall_core::{GroupName, SystemProbe};
    let system = snapall_system::LinuxSystem::new();
    let mut made = system.groups()?;
    let protected = ctx.backend.list_configs()?;
    match action {
        None | Some("list") => ctx.show.result(
            render::groups(&made, &protected),
            render::groups_json(&made, &protected),
        ),
        Some("add") => {
            let name =
                GroupName::parse(name.ok_or("which group? `snapall groups add NAME AREA,AREA`")?)?;
            let wanted = areas.ok_or("which areas? `snapall groups add NAME AREA,AREA`")?;
            let list = crate::cli::parse_only(wanted)?;
            if let Some(unprotected) = list.iter().find(|area| !protected.contains(area)) {
                let there: Vec<String> = protected.iter().map(ToString::to_string).collect();
                return Err(format!(
                    "`{unprotected}` is not a protected area (there are: {})",
                    there.join(", ")
                )
                .into());
            }
            let before = made.get(&name).map(<[ConfigName]>::to_vec);
            let question = format!(
                "Make the group {name} of {}?",
                list.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if let Some(stop) = ask::gate(Act::ChangeGroups.weight(), yes, &question, None)? {
                return Ok(stop);
            }
            let change = Change::begin(
                &format!("Making the group {name}"),
                &format!("Made the group {name}"),
                None,
            )?;
            let saved = made
                .set(name.clone(), list)
                .map_err(|e| e.to_string())
                .and_then(|()| system.save_groups(&made));
            if saved.is_ok() {
                change.rollback(snapall_core::rollback::after_groups(
                    &name,
                    before.as_deref(),
                ));
            }
            change.end(saved.clone().map(|()| String::new()));
            saved?;
            ctx.show.result(
                format!(
                    "The group {name} is now: {}
",
                    made.get(&name).map_or(String::new(), |a| a
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "))
                ),
                serde_json::json!({ "group": name.to_string(), "changed": true }),
            )
        }
        Some("remove") => {
            let name = GroupName::parse(name.ok_or("which group? `snapall groups remove NAME`")?)?;
            let before = made.get(&name).map(<[ConfigName]>::to_vec);
            if before.is_none() {
                return Err(format!("there is no group called `{name}`").into());
            }
            let question =
                format!("Take the group {name} away? The areas and their snapshots are untouched.");
            if let Some(stop) = ask::gate(Act::ChangeGroups.weight(), yes, &question, None)? {
                return Ok(stop);
            }
            made.remove(&name);
            let change = Change::begin(
                &format!("Removing the group {name}"),
                &format!("Removed the group {name}"),
                None,
            )?;
            let saved = system.save_groups(&made);
            if saved.is_ok() {
                change.rollback(snapall_core::rollback::after_groups(
                    &name,
                    before.as_deref(),
                ));
            }
            change.end(saved.clone().map(|()| String::new()));
            saved?;
            ctx.show.result(
                format!(
                    "The group {name} is gone. The areas and their snapshots are untouched.
"
                ),
                serde_json::json!({ "group": name.to_string(), "changed": true }),
            )
        }
        Some(other) => Err(format!("`{other}` is not `add` or `remove`").into()),
    }
}

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
    if let Some(stop) = ask::gate(Act::RoomCounting.weight(), yes, &question, None)? {
        return Ok(stop);
    }
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
