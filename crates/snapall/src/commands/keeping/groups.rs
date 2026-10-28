// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Groups of areas: naming a set of them to snapshot together.

use super::*;
use crate::output::Outcome;
use crate::{ask, render};
use snapall_core::{Act, AreaName, GroupName, SystemProbe};
use std::error::Error;

/// The areas of a group, out of the ones protected now; a group that names one that is not is refused.
pub(crate) fn areas_of_group(ctx: &Context, wanted: &str) -> Result<Vec<AreaName>, Box<dyn Error>> {
    use snapall_core::{SystemProbe, Wanted};
    let wanted = Wanted::parse(wanted)?;
    let groups = snapall_system::LinuxSystem::new().groups()?;
    let protected = ctx.backend.list_areas()?;
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
    let system = snapall_system::LinuxSystem::new();
    match action {
        None | Some("list") => list_the_groups(ctx, &system),
        Some("add") => make_a_group(ctx, &system, name, areas, yes),
        Some("remove") => take_a_group_away(ctx, &system, name, yes),
        Some(other) => Err(format!("`{other}` is not `add` or `remove`").into()),
    }
}

fn list_the_groups(ctx: &Context, system: &dyn SystemProbe) -> Outcome {
    let made = system.groups()?;
    let protected = ctx.backend.list_areas()?;
    ctx.show.result(
        render::groups(&made, &protected),
        render::groups_json(&made, &protected),
    )
}

fn make_a_group(
    ctx: &Context,
    system: &dyn SystemProbe,
    name: Option<&str>,
    areas: Option<&str>,
    yes: bool,
) -> Outcome {
    let mut made = system.groups()?;
    let protected = ctx.backend.list_areas()?;
    let (name, list) = the_group_asked_for(name, areas, &protected)?;
    let before = made.get(&name).map(<[AreaName]>::to_vec);
    let question = format!("Make the group {name} of {}?", in_words(&list));
    let approved = ask::gate(Act::ChangeGroups, yes, &question, None)?;
    let change = Change::begin(
        approved,
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
    change.end_with(&saved);
    saved?;
    ctx.show.result(
        format!(
            "The group {name} is now: {}\n",
            in_words(made.get(&name).unwrap_or(&[]))
        ),
        serde_json::json!({ "group": name.to_string(), "changed": true }),
    )
}

fn take_a_group_away(
    ctx: &Context,
    system: &dyn SystemProbe,
    name: Option<&str>,
    yes: bool,
) -> Outcome {
    let mut made = system.groups()?;
    let name = GroupName::parse(name.ok_or("which group? `snapall groups remove NAME`")?)?;
    let before = made.get(&name).map(<[AreaName]>::to_vec);
    if before.is_none() {
        return Err(format!("there is no group called `{name}`").into());
    }
    let question =
        format!("Take the group {name} away? The areas and their snapshots are untouched.");
    let approved = ask::gate(Act::ChangeGroups, yes, &question, None)?;
    made.remove(&name);
    let change = Change::begin(
        approved,
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
    change.end_with(&saved);
    saved?;
    ctx.show.result(
        format!("The group {name} is gone. The areas and their snapshots are untouched.\n"),
        serde_json::json!({ "group": name.to_string(), "changed": true }),
    )
}

/// The group named and its areas, out of what was typed: both must be given, and every area must be protected.
fn the_group_asked_for(
    name: Option<&str>,
    areas: Option<&str>,
    protected: &[AreaName],
) -> Result<(GroupName, Vec<AreaName>), Box<dyn Error>> {
    let name = GroupName::parse(name.ok_or("which group? `snapall groups add NAME AREA,AREA`")?)?;
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
    Ok((name, list))
}

fn in_words(areas: &[AreaName]) -> String {
    let words: Vec<String> = areas.iter().map(ToString::to_string).collect();
    words.join(", ")
}
