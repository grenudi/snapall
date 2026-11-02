// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Putting every area back to how a restore point had them.

use super::*;
use crate::output;
use crate::output::Outcome;
use crate::{ask, render};
use snapall_core::going_back::boot;
use snapall_core::Act;
use snapall_core::{
    all_snapshots, preview_point, restore_point_along, restore_points, MemberView, Point,
    RestorePoint,
};
use snapall_system::boot_menu;
use std::error::Error;
use std::process::ExitCode;

/// Put every area back to one restore point with a single action: shows what would change in each,
/// asks first (unless told not to), takes a safety snapshot per area (all one restore point, so
/// that undoing this is one action too) and reports each area. The running system is left alone.
pub fn restore_all(
    ctx: &Context,
    wanted: &Point,
    mode: Mode,
    agreed: Agreed,
    job: Option<&str>,
) -> Outcome {
    let found = restore_points(&all_snapshots(ctx.backend)?);
    let point = the_point_named(&found, wanted)?;
    let previews = preview_point(ctx.backend, point);
    let route = boot::restore_route(&boot::assess(&boot_menu::on_this_machine()));
    if mode.shows_only() || !anything_to_change(&previews) {
        return ctx.show.result(
            render::point_preview(point, &previews, &route),
            render::point_preview_json(point, &previews),
        );
    }
    let approved = ask_before_putting_back(agreed, point, &previews, &route)?;
    let change = Change::begin(
        approved,
        &format!("Putting every area back to restore point {}", point.point),
        &format!("Put every area back to restore point {}", point.point),
        job,
    )?;
    let done = put_every_area_back(ctx, point, &change);
    end_the_change(change, point, &done);
    ctx.show.result(
        render::point_restored(point, &done),
        render::point_restored_json(point, &done),
    )?;
    Ok(if done.succeeded() && !done.stopped {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn anything_to_change(previews: &[snapall_core::MemberPreview]) -> bool {
    previews
        .iter()
        .any(|p| matches!(&p.view, MemberView::Changes(c) if !c.changes.is_empty()))
}

/// Area by area, each after its own safety snapshot; the way back is written down as soon as there is one.
fn put_every_area_back(
    ctx: &Context,
    point: &RestorePoint,
    change: &Change,
) -> snapall_core::PointRestore {
    let done = restore_point_along(
        ctx.backend,
        point,
        a_restore_point_for_now(ctx.backend),
        &change.course(),
    );
    if !done.members.is_empty() {
        change.rollback(snapall_core::going_back::rollback::after_restoring_all(
            done.safety.as_str(),
        ));
    }
    done
}

/// Stopped on request or not, the record says how it went.
fn end_the_change(change: Change, point: &RestorePoint, done: &snapall_core::PointRestore) {
    if done.stopped {
        let words = done.stopped_words(point.members.len());
        output::note(&words.now);
        change.end_stopped(words.record);
    } else {
        change.end(done.how_it_went());
    }
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

fn restore_point_question(
    point: &RestorePoint,
    previews: &[snapall_core::MemberPreview],
) -> String {
    let areas = previews
        .iter()
        .filter(|p| matches!(&p.view, MemberView::Changes(c) if !c.changes.is_empty()))
        .count();
    format!(
        "Put {} back to restore point {}? Each one's current state is snapshotted first, so this can be undone.",
        snapall_core::vocabulary::explain::counted(areas, "area"),
        point.point
    )
}

fn ask_before_putting_back(
    agreed: Agreed,
    point: &RestorePoint,
    previews: &[snapall_core::MemberPreview],
    route: &str,
) -> Result<ask::Approved, Box<dyn std::error::Error>> {
    ask::before_showing(
        Act::RestorePoint,
        agreed,
        || {
            format!(
                "{}\n{}\n",
                render::point_preview(point, previews, route),
                snapall_core::vocabulary::explain::RESTORE_CAUTION
            )
        },
        &restore_point_question(point, previews),
    )
}
