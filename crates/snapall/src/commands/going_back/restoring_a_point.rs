// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting every area back to how a restore point had them.

use super::*;
use crate::output::Outcome;
use crate::{ask, boot_probe, render};
use snapall_core::boot;
use snapall_core::{
    all_snapshots, fresh_point, preview_point, restore_point_along, restore_points, MemberView,
    Point, RestorePoint,
};
use std::error::Error;
use std::process::ExitCode;

/// Put every area back to one restore point with a single action: shows what would change in each,
/// asks first (unless told not to), takes a safety snapshot per area (all one restore point, so
/// that undoing this is one action too) and reports each area. The running system is left alone.
pub fn restore_all(
    ctx: &Context,
    wanted: &Point,
    dry_run: bool,
    yes: bool,
    job: Option<&str>,
) -> Outcome {
    let found = restore_points(&all_snapshots(ctx.backend)?);
    let point = the_point_named(&found, wanted)?;
    let previews = preview_point(ctx.backend, point);
    let route = boot::restore_route(&boot::assess(&boot_probe::system()));
    let anything_to_change = previews
        .iter()
        .any(|p| matches!(&p.view, MemberView::Changes(c) if !c.changes.is_empty()));
    if dry_run || !anything_to_change {
        return ctx.show.result(
            render::point_preview(point, &previews, &route),
            render::point_preview_json(point, &previews),
        );
    }
    match point_confirmation(yes, point, &previews, &route) {
        Confirmation::Given => {}
        Confirmation::NoTerminalToAskOn => return Ok(ExitCode::from(2)),
        Confirmation::Declined => return Ok(ExitCode::FAILURE),
    }
    let change = Change::begin(
        &format!("Putting every area back to restore point {}", point.point),
        &format!("Put every area back to restore point {}", point.point),
        job,
    )?;
    let done = restore_point_along(
        ctx.backend,
        point,
        fresh_point(ctx.backend, unix_now()),
        &change.course(),
    );
    if !done.members.is_empty() {
        change.rollback(snapall_core::rollback::after_restoring_all(
            done.safety.as_str(),
        ));
    }
    if done.stopped {
        let (put_back, total) = (done.members.len(), point.members.len());
        eprintln!("snapall: stopped on request after {put_back} of {total} areas; the rest were left as they are");
        change.end_stopped(format!(
            "stopped on request after {put_back} of {total} areas: the others were left as they are. To undo what was done: restore point {}",
            done.safety.as_str()
        ));
        ctx.show.result(
            render::point_restored(point, &done),
            render::point_restored_json(point, &done),
        )?;
        return Ok(ExitCode::FAILURE);
    }
    change.end(if done.succeeded() {
        let put_back = done
            .members
            .iter()
            .filter(|m| matches!(m.result, snapall_core::MemberResult::Restored(_)))
            .count();
        Ok(if done.changed_anything() {
            format!(
                "{put_back} area{} put back. To undo it: restore point {}",
                if put_back == 1 { "" } else { "s" },
                done.safety.as_str()
            )
        } else {
            String::new()
        })
    } else {
        Err("some areas could not be put back: the output says which".to_owned())
    });
    ctx.show.result(
        render::point_restored(point, &done),
        render::point_restored_json(point, &done),
    )?;
    Ok(if done.succeeded() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

pub(super) fn the_point_named<'a>(
    found: &'a [RestorePoint],
    wanted: &Point,
) -> Result<&'a RestorePoint, Box<dyn Error>> {
    found.iter().find(|p| p.point == *wanted).ok_or_else(|| {
        if found.is_empty() {
            "There are no restore points yet. `sudo snapall` takes a snapshot of every area at once, and that is one.".into()
        } else {
            format!("`{wanted}` is not a restore point. `snapall points` lists them.").into()
        }
    })
}

/// Putting areas back changes files, so the person is asked, unless they said `--yes`.
pub(super) fn point_confirmation(
    yes: bool,
    point: &RestorePoint,
    previews: &[snapall_core::MemberPreview],
    route: &str,
) -> Confirmation {
    if yes {
        return Confirmation::Given;
    }
    if !ask::can_ask() {
        eprintln!(
            "snapall: restore-all changes files and there is no terminal to ask on; \
             look first with --dry-run, then pass --yes"
        );
        return Confirmation::NoTerminalToAskOn;
    }
    eprint!("{}", render::point_preview(point, previews, route));
    eprintln!("\n{}\n", snapall_core::explain::RESTORE_CAUTION);
    let areas = previews
        .iter()
        .filter(|p| matches!(&p.view, MemberView::Changes(c) if !c.changes.is_empty()))
        .count();
    let question = format!(
        "Put {areas} area{} back to restore point {}? Each one's current state is snapshotted first, so this can be undone.",
        if areas == 1 { "" } else { "s" },
        point.point
    );
    match ask::yes_no(&question) {
        Ok(true) => Confirmation::Given,
        _ => {
            eprintln!("snapall: not restoring");
            Confirmation::Declined
        }
    }
}
