// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Everything that can be asked for, and the command that does it.

use super::*;
use crate::cli::Request;
use crate::output::Outcome;
use crate::{doctor, generate};

pub fn carry_out(ctx: &Context, request: Request) -> Outcome {
    match request {
        Request::Help => about::help(ctx),
        Request::Version => about::version(ctx),
        Request::Contract => about::contract(ctx),
        Request::Completions { shell } => generate::completions(ctx.show, shell),
        Request::Man { source_date_epoch } => generate::man_page(ctx.show, source_date_epoch),

        Request::List => looking::list(ctx),
        Request::Snapshots { area } => looking::snapshots(ctx, area),
        Request::History { path } => looking::history(ctx, &path),
        Request::Changes { area, id, path } => looking::changes(ctx, &area, id, path.as_deref()),
        Request::Points => looking::points(ctx),
        Request::Health => looking::health(ctx),
        Request::Activity => looking::activity(ctx),

        Request::Create {
            only,
            description,
            dry_run,
            job,
            group,
        } => taking::create(
            ctx,
            only.as_deref(),
            group.as_deref(),
            &description,
            dry_run,
            job.as_deref(),
        ),

        Request::Delete { area, id, destroy } => deleting::delete(ctx, &area, id, destroy),

        Request::Restore {
            area,
            id,
            path,
            dry_run,
            yes,
        } => going_back::restore(ctx, &area, id, path.as_deref(), dry_run, yes),
        Request::RestoreAll {
            point,
            yes,
            dry_run,
            job,
        } => going_back::restore_all(ctx, &point, dry_run, yes, job.as_deref()),
        Request::Undo {
            change,
            yes,
            destroy,
        } => going_back::undo(ctx, &change, yes, destroy),

        Request::Setup { area, yes, dry_run } => {
            protecting::setup(ctx, area.as_deref(), yes, dry_run)
        }
        Request::Unprotect {
            area,
            dry_run,
            destroy,
        } => protecting::unprotect(ctx, &area, dry_run, destroy),
        Request::Doctor { install, yes } => {
            doctor::run(ctx.backend, ctx.observer, install, yes, ctx.json)
        }

        Request::Limits {
            area,
            keep,
            min_age,
            cleanup,
            yes,
            destroy,
        } => {
            let wanted = keeping::LimitsWanted {
                keep: keep.as_deref(),
                min_age: min_age.as_deref(),
                cleanup: cleanup.as_deref(),
            };
            keeping::limits(ctx, &area, wanted, yes, destroy)
        }
        Request::Groups {
            action,
            name,
            areas,
            yes,
        } => keeping::groups(
            ctx,
            action.as_deref(),
            name.as_deref(),
            areas.as_deref(),
            yes,
        ),
        Request::Quotas { state, yes } => keeping::quotas(ctx, state.as_deref(), yes),

        Request::Cancel { job, yes } => running::cancel(ctx, &job, yes),
        Request::Tui { file } => window::tui(ctx, file.as_deref()),
    }
}
