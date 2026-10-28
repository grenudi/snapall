// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Taking snapshots: of one area, of every protected area, or of a group.

use super::*;
use crate::ask;
use crate::boot_probe;
use crate::output::{self, Outcome};
use crate::render;
use snapall_core::going_back::boot;
use snapall_core::Act;
use snapall_core::{
    fresh_point, plan_snapshots, run_plan_along, AreaName, Description, Plan, Report,
};
use std::io::IsTerminal;
use std::process::ExitCode;

pub fn create(
    ctx: &Context,
    only: Option<&[AreaName]>,
    group: Option<&str>,
    description: &Description,
    dry_run: bool,
    job: Option<&str>,
) -> Outcome {
    let chosen = the_areas_asked_for(ctx, only, group)?;
    let plan = plan_snapshots(ctx.backend, chosen.as_deref().or(only))?;
    if dry_run {
        return ctx.show.result(
            render::dry_run(&plan, description),
            render::dry_run_json(&plan, description),
        );
    }
    let areas = areas_in_words(&plan);
    let approved = ask::nothing_to_ask(Act::TakeSnapshot)?;
    let change = Change::begin(
        approved,
        &format!("Taking a snapshot of {areas}"),
        &format!("Took a snapshot of {areas}"),
        job,
    )?;
    // every snapshot of this one action is the same restore point
    let point = fresh_point(ctx.backend, unix_now());
    let (report, stopped) = run_plan_along(
        ctx.backend,
        &plan,
        description,
        Some(&point),
        &change.course(),
    );
    if let Some(way_back) = snapall_core::going_back::rollback::after_taking(&report) {
        change.rollback(way_back);
    }
    record_how_taking_went(change, &plan, &report, stopped);
    say_what_was_taken(ctx, &plan, &report, stopped)
}

/// A group is a name for a set of areas: it is what `--only` would have been.
fn the_areas_asked_for(
    ctx: &Context,
    only: Option<&[AreaName]>,
    group: Option<&str>,
) -> Result<Option<Vec<AreaName>>, Box<dyn std::error::Error>> {
    match (group, only) {
        (Some(_), Some(_)) => Err("use either --only or --group, not both".into()),
        (Some(wanted), None) => Ok(Some(areas_of_group(ctx, wanted)?)),
        (None, _) => Ok(None),
    }
}

fn areas_in_words(plan: &Plan) -> String {
    let areas: Vec<String> = plan.targets().iter().map(ToString::to_string).collect();
    areas.join(", ")
}

fn record_how_taking_went(change: Change, plan: &Plan, report: &Report, stopped: bool) {
    let (taken, total) = (report.outcomes.len(), plan.targets().len());
    if stopped {
        change.end_stopped(format!(
            "stopped on request after {taken} of {total} areas: the others were not started"
        ));
    } else if report.is_success() {
        change.end(Ok(format!(
            "{taken} snapshot{} taken",
            if taken == 1 { "" } else { "s" }
        )));
    } else {
        change.end(Err(what_failed(report)));
    }
}

fn what_failed(report: &Report) -> String {
    let failures: Vec<String> = report
        .outcomes
        .iter()
        .filter_map(|o| {
            o.result
                .as_ref()
                .err()
                .map(|why| format!("{}: {why}", o.area))
        })
        .collect();
    failures.join("; ")
}

fn say_what_was_taken(ctx: &Context, plan: &Plan, report: &Report, stopped: bool) -> Outcome {
    if stopped {
        let (taken, total) = (report.outcomes.len(), plan.targets().len());
        output::note(&format!(
            "snapall: stopped on request after {taken} of {total} areas; the rest were not started"
        ));
    }
    ctx.show
        .result(render::report(report), render::report_json(report))?;
    if stopped || !report.is_success() {
        return Ok(ExitCode::FAILURE);
    }
    suggest_the_boot_menu(ctx);
    Ok(ExitCode::SUCCESS)
}

/// After a snapshot: a nudge to a person, on stderr (stdout stays parseable), and only when a
/// boot-menu tool would help.
pub(super) fn suggest_the_boot_menu(ctx: &Context) {
    let to_a_person = std::io::stderr().is_terminal() && !ctx.show.is_json();
    if !ctx.hints || !to_a_person {
        return;
    }
    if let Some(tip) = boot::hint(&boot::assess(&boot_probe::system())) {
        output::note(&tip);
    }
}
