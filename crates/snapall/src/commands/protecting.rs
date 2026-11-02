// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Protecting parts of the disk, and stopping protecting them.

use super::*;
use crate::output;
use crate::output::Outcome;
use crate::{ask, render, setup};
use snapall_core::{Act, AreaName};
use std::error::Error;
use std::process::ExitCode;

/// Stop protecting an area: every snapshot of it is deleted for good, its own files are not touched. It
/// always says what that means first; on a terminal it needs the area's name typed out, in a script `--yes`;
/// `--dry-run` only says. The system area is refused.
pub fn unprotect(ctx: &Context, area: &str, mode: Mode, agreed: Agreed) -> Outcome {
    use snapall_core::protecting::unprotect::{plan, stop_protecting};
    use snapall_core::SystemProbe;
    let area_name = AreaName::parse(area)?;
    let system = snapall_system::LinuxSystem::new();
    let groups = system.groups()?;
    let plan = plan(ctx.backend, &groups, &area_name)?;
    output::note(&format!("{}\n", plan.words().join("\n")));
    if mode.shows_only() {
        return ctx.show.result(
            "Dry run: nothing was changed.
"
            .to_owned(),
            serde_json::json!({ "area": area, "snapshots": plan.snapshots, "changed": false }),
        );
    }
    // the word and the area's name: the word says it is understood, the name that it is this one
    let question = format!(
        "Stop protecting {area} and delete its {} snapshots for good?",
        plan.snapshots
    );
    let approved = ask::gate(
        Act::StopProtecting,
        agreed,
        &question,
        Some(area_name.as_str()),
    )?;
    let title = snapall_core::protecting::coverage::describe(&plan.location).title;
    let change = Change::begin(
        approved,
        &format!("Stopping protection of {title}"),
        &format!("Stopped protecting {title}"),
        None,
    )?;
    let result = stop_protecting(ctx.backend, &plan).map_err(|e| e.to_string());
    if result.is_ok() {
        change.rollback(snapall_core::Rollback::Advice(
            snapall_core::vocabulary::explain::UNPROTECT_ADVICE.to_owned(),
        ));
    }
    change.end(result.clone().map(|n| format!("{n} snapshots deleted")));
    let deleted = result?;
    ctx.show.result(
        format!(
            "{title} is no longer protected. {deleted} snapshot{} deleted; its files are untouched.
",
            if deleted == 1 { " was" } else { "s were" }
        ),
        serde_json::json!({ "area": area, "snapshots_deleted": deleted, "changed": true }),
    )
}

/// Protect the parts of the disk that snapshots do not cover yet: explain, ask, and do it. With
/// `--json` nothing is said or asked, and nothing is changed unless `--yes` is given as well.
pub fn setup(ctx: &Context, area: Option<&str>, agreed: Agreed, mode: Mode) -> Outcome {
    let json = ctx.show.is_json();
    let options = options_for(area, agreed, mode, json);
    let change = begin_unless_rehearsing(&options)?;
    let report = hold_the_conversation(ctx, &options, json)?;
    if let Some(change) = change {
        end_with_the_way_back(change, &report);
    }
    if json {
        ctx.show.json(&render::setup_json(&report))?;
    }
    Ok(if report.succeeded() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// What was asked on the command line. A program reading the output (`--json`) cannot be asked anything, so
/// without `--yes` it is shown what would be done and nothing is.
fn options_for(area: Option<&str>, agreed: Agreed, mode: Mode, json: bool) -> setup::Options {
    let yes = agreed.covers(snapall_core::Weight::Changes);
    setup::Options {
        area: area.map(str::to_owned),
        yes,
        dry_run: mode.shows_only() || (json && !yes),
    }
}

/// A rehearsal makes no change; anything else does, and it asks as it goes, area by area.
fn begin_unless_rehearsing(options: &setup::Options) -> Result<Option<Change>, Box<dyn Error>> {
    if options.dry_run {
        return Ok(None);
    }
    Change::begin_asking_along(
        "Protecting a part of the disk",
        "Protected a part of the disk",
        None,
    )
    .map(Some)
}

/// To a person on the terminal, or without a word for a program that reads the output.
fn hold_the_conversation(
    ctx: &Context,
    options: &setup::Options,
    json: bool,
) -> Result<setup::Report, Box<dyn Error>> {
    let facts = setup::gather();
    if json {
        setup::run(ctx.backend, &facts, &mut setup::Quiet, options)
    } else {
        setup::run(ctx.backend, &facts, &mut setup::Terminal, options)
    }
}

fn end_with_the_way_back(change: Change, report: &setup::Report) {
    if !report.protected_now.is_empty() {
        let names: Vec<String> = report
            .protected_now
            .iter()
            .map(|(_, name)| name.to_string())
            .collect();
        change.rollback(snapall_core::going_back::rollback::after_protecting(&names));
    }
    change.end(if report.succeeded() {
        Ok(String::new())
    } else {
        Err("not everything could be protected: the output says what".to_owned())
    });
}
