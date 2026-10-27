// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Protecting parts of the disk, and stopping protecting them.

use super::*;
use crate::output::Outcome;
use crate::{ask, render, setup};
use snapall_core::{Act, ConfigName};
use std::process::ExitCode;

/// Stop protecting an area: every snapshot of it is deleted for good, its own files are not touched. It
/// always says what that means first; on a terminal it needs the area's name typed out, in a script `--yes`;
/// `--dry-run` only says. The system area is refused.
pub fn unprotect(ctx: &Context, area: &str, dry_run: bool, destroy: bool) -> Outcome {
    use snapall_core::unprotect::{plan, stop_protecting};
    use snapall_core::SystemProbe;
    let config = ConfigName::parse(area)?;
    let system = snapall_system::LinuxSystem::new();
    let groups = system.groups()?;
    let plan = plan(ctx.backend, &groups, &config)?;
    eprintln!(
        "{}
",
        plan.words().join(
            "
"
        )
    );
    if dry_run {
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
    if let Some(stop) = ask::gate(
        Act::StopProtecting.weight(),
        destroy,
        &question,
        Some(config.as_str()),
    )? {
        return Ok(stop);
    }
    let title = snapall_core::coverage::describe(&plan.subvolume).title;
    let change = Change::begin(
        &format!("Stopping protection of {title}"),
        &format!("Stopped protecting {title}"),
        None,
    )?;
    let result = stop_protecting(ctx.backend, &plan).map_err(|e| e.to_string());
    if result.is_ok() {
        change.rollback(snapall_core::Rollback::Advice(
            snapall_core::explain::UNPROTECT_ADVICE.to_owned(),
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
pub fn setup(ctx: &Context, area: Option<&str>, yes: bool, dry_run: bool) -> Outcome {
    let json = ctx.show.is_json();
    let options = setup::Options {
        area: area.map(str::to_owned),
        yes,
        dry_run: dry_run || (json && !yes),
    };
    let change = if options.dry_run {
        None
    } else {
        Some(Change::begin(
            "Protecting a part of the disk",
            "Protected a part of the disk",
            None,
        )?)
    };
    let facts = setup::gather();
    let report = if json {
        setup::run(ctx.backend, &facts, &mut setup::Quiet, &options)?
    } else {
        setup::run(ctx.backend, &facts, &mut setup::Terminal, &options)?
    };
    if let Some(change) = change {
        if !report.protected_now.is_empty() {
            let names: Vec<String> = report
                .protected_now
                .iter()
                .map(|(_, name)| name.to_string())
                .collect();
            change.rollback(snapall_core::rollback::after_protecting(&names));
        }
        change.end(if report.succeeded() {
            Ok(String::new())
        } else {
            Err("not everything could be protected: the output says what".to_owned())
        });
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
