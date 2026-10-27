// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Groups of areas: naming a set of them to snapshot together.

use super::*;
use crate::output::Outcome;
use crate::{ask, render};
use snapall_core::{Act, ConfigName};
use std::error::Error;

/// The areas of a group, out of the ones protected now; a group that names one that is not is refused.
pub(crate) fn areas_of_group(
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
            ask::gate(Act::ChangeGroups.weight(), yes, &question, None)?;
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
            ask::gate(Act::ChangeGroups.weight(), yes, &question, None)?;
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
