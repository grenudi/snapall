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
            Mode::from_dry_run(dry_run),
            job.as_deref(),
        ),

        Request::Delete { area, id, destroy } => {
            deleting::delete(ctx, &area, id, Agreed::by_destroy(destroy))
        }

        Request::Restore {
            area,
            id,
            path,
            dry_run,
            yes,
        } => going_back::restore(
            ctx,
            &area,
            id,
            path.as_deref(),
            Mode::from_dry_run(dry_run),
            Agreed::by_yes(yes),
        ),
        Request::RestoreAll {
            point,
            yes,
            dry_run,
            job,
        } => going_back::restore_all(
            ctx,
            &point,
            Mode::from_dry_run(dry_run),
            Agreed::by_yes(yes),
            job.as_deref(),
        ),
        Request::Undo {
            change,
            yes,
            destroy,
        } => going_back::undo(ctx, &change, Agreed::to(yes, destroy)),

        Request::Setup { area, yes, dry_run } => protecting::setup(
            ctx,
            area.as_deref(),
            Agreed::by_yes(yes),
            Mode::from_dry_run(dry_run),
        ),
        Request::Unprotect {
            area,
            dry_run,
            destroy,
        } => protecting::unprotect(
            ctx,
            &area,
            Mode::from_dry_run(dry_run),
            Agreed::by_destroy(destroy),
        ),
        Request::Doctor { install, yes } => {
            doctor::run(ctx.backend, ctx.observer, install, yes, ctx.show.is_json())
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
            keeping::limits(ctx, &area, wanted, Agreed::to(yes, destroy))
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
            Agreed::by_yes(yes),
        ),
        Request::Quotas { state, yes } => {
            keeping::quotas(ctx, state.as_deref(), Agreed::by_yes(yes))
        }

        Request::Cancel { job, yes } => running::cancel(ctx, &job, Agreed::by_yes(yes)),
        Request::Tui { file } => window::tui(ctx, file.as_deref()),
    }
}
