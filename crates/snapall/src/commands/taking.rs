// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Taking snapshots: of one area, of every protected area, or of a group.

use super::*;
use crate::boot_probe;
use crate::output::Outcome;
use crate::render;
use snapall_core::boot;
use snapall_core::{fresh_point, plan_snapshots, run_plan_along, ConfigName, Description};
use std::io::IsTerminal;
use std::process::ExitCode;

pub fn create(
    ctx: &Context,
    only: Option<&[ConfigName]>,
    group: Option<&str>,
    description: &Description,
    dry_run: bool,
    job: Option<&str>,
) -> Outcome {
    // a group is a name for a set of areas: it is what `--only` would have been
    let from_group = match group {
        Some(wanted) => {
            if only.is_some() {
                return Err("use either --only or --group, not both".into());
            }
            Some(areas_of_group(ctx, wanted)?)
        }
        None => None,
    };
    let only = from_group.as_deref().or(only);
    let plan = plan_snapshots(ctx.backend, only)?;
    if dry_run {
        return ctx.show.result(
            render::dry_run(&plan, description),
            render::dry_run_json(&plan, description),
        );
    }
    let areas = plan
        .targets()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let change = Change::begin(
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
    if let Some(way_back) = snapall_core::rollback::after_taking(&report) {
        change.rollback(way_back);
    }
    if stopped {
        let (taken, total) = (report.outcomes.len(), plan.targets().len());
        eprintln!(
            "snapall: stopped on request after {taken} of {total} areas; the rest were not started"
        );
        let how_far = format!(
            "stopped on request after {taken} of {total} areas: the others were not started"
        );
        change.end_stopped(how_far);
        ctx.show
            .result(render::report(&report), render::report_json(&report))?;
        return Ok(ExitCode::FAILURE);
    }
    change.end(if report.is_success() {
        let taken = report.outcomes.len();
        Ok(format!(
            "{taken} snapshot{} taken",
            if taken == 1 { "" } else { "s" }
        ))
    } else {
        Err(report
            .outcomes
            .iter()
            .filter_map(|o| {
                o.result
                    .as_ref()
                    .err()
                    .map(|why| format!("{}: {why}", o.config))
            })
            .collect::<Vec<_>>()
            .join("; "))
    });
    ctx.show
        .result(render::report(&report), render::report_json(&report))?;
    if !report.is_success() {
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
        eprintln!("{tip}");
    }
}
