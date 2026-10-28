// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How many snapshots an area keeps, and when its oldest go.

use super::*;
use crate::ask;
use crate::output;
use crate::output::Outcome;
use snapall_core::{
    limits_words, preview_limits, Act, AreaName, Limits, LimitsPreview, SystemProbe,
};
use std::error::Error;

/// What was asked for, as typed: each may be absent.
pub struct LimitsWanted<'a> {
    pub keep: Option<&'a str>,
    pub min_age: Option<&'a str>,
    pub cleanup: Option<&'a str>,
}

/// How many snapshots an area keeps, and when its oldest go. Without options it shows them and what the
/// next cleanup would remove; with options it changes them, after saying what that would remove (a removed
/// snapshot cannot be brought back, so a change that removes any asks first, or needs `--destroy`).
pub fn limits(
    ctx: &Context,
    area: &str,
    wanted: LimitsWanted<'_>,
    yes: bool,
    destroy: bool,
) -> Outcome {
    let config = a_protected_area(ctx, area)?;
    let before = the_limits_now(ctx, &config)?;
    let after = the_limits_wanted(&before, &wanted)?;
    let preview = what_the_next_cleanup_would_remove(ctx, &config, &after)?;
    let options = after.changes_from(&before);
    if options.is_empty() {
        return say_the_limits(ctx, area, &config, &before, &preview, false);
    }
    output::note(&what_changing_would_do(&config, &before, &after, &preview));
    let approved = ask_before_changing(&preview, yes, destroy)?;
    let change = Change::begin(
        approved,
        &format!("Changing how many snapshots {config} keeps"),
        &format!("Changed how many snapshots {config} keeps"),
        None,
    )?;
    let result = set_the_options(ctx, &config, &options);
    if result.is_ok() {
        change.rollback(snapall_core::rollback::after_limits(
            &config.to_string(),
            &before,
        ));
    }
    change.end_with(&result);
    result?;
    say_the_limits(ctx, area, &config, &after, &preview, true)
}

fn a_protected_area(ctx: &Context, area: &str) -> Result<AreaName, Box<dyn Error>> {
    let named = AreaName::parse(area)?;
    if !ctx.backend.list_areas()?.contains(&named) {
        return Err(format!("`{area}` is not a protected area: `snapall list` shows them").into());
    }
    Ok(named)
}

fn the_limits_now(ctx: &Context, config: &AreaName) -> Result<Limits, Box<dyn Error>> {
    let kept = Limits::from_retention(&ctx.backend.retention(config)?);
    kept.ok_or_else(|| {
        "snapper did not say how many this area keeps, so there is nothing to show or change".into()
    })
}

/// What was typed, laid over what is: each part that was given must be a sensible value.
fn the_limits_wanted(before: &Limits, wanted: &LimitsWanted<'_>) -> Result<Limits, Box<dyn Error>> {
    let mut after = *before;
    if let Some(text) = wanted.keep {
        after.keep =
            u32::try_from(a_whole_number(text, "--keep")?).map_err(|_| "--keep is too large")?;
    }
    if let Some(text) = wanted.min_age {
        after.min_age_seconds = a_whole_number(text, "--min-age")? * 60;
    }
    match wanted.cleanup {
        Some("on") => after.cleanup = true,
        Some("off") => after.cleanup = false,
        Some(other) => return Err(format!("--cleanup must be `on` or `off`, not `{other}`").into()),
        None => {}
    }
    match after.problem() {
        Some(problem) => Err(problem.into()),
        None => Ok(after),
    }
}

fn a_whole_number(text: &str, what: &str) -> Result<u64, Box<dyn Error>> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| format!("{what} must be a whole number, not `{text}`").into())
}

fn what_the_next_cleanup_would_remove(
    ctx: &Context,
    config: &AreaName,
    after: &Limits,
) -> Result<LimitsPreview, Box<dyn Error>> {
    let now = snapall_system::LinuxSystem::new()
        .local_time()
        .as_deref()
        .and_then(snapall_core::calendar::seconds_from_snapshot_date);
    Ok(preview_limits(
        &ctx.backend.list_snapshots(config)?,
        after,
        now,
    ))
}

/// A change that would remove snapshots is destroying, and any other is a change like the rest.
fn ask_before_changing(
    preview: &LimitsPreview,
    yes: bool,
    destroy: bool,
) -> Result<ask::Approved, Box<dyn Error>> {
    let removes = preview.would_remove > 0;
    ask::gate(
        Act::ChangeLimits { removes },
        if removes { destroy } else { yes },
        "Apply this?",
        None,
    )
}

fn set_the_options(
    ctx: &Context,
    config: &AreaName,
    options: &[snapall_core::AreaOption],
) -> Result<(), String> {
    options.iter().try_for_each(|option| {
        ctx.backend
            .set_area_option(config, *option)
            .map_err(|e| e.to_string())
    })
}

fn describe(limits: &Limits) -> String {
    format!(
        "keeps the newest {}, never removes one younger than {} minutes, cleanup {}",
        limits.keep,
        limits.min_age_seconds / 60,
        if limits.cleanup { "on" } else { "off" }
    )
}

fn what_changing_would_do(
    config: &AreaName,
    before: &Limits,
    after: &Limits,
    preview: &LimitsPreview,
) -> String {
    format!(
        "{config} currently {}.\nAfter: {}.\n{}\n",
        describe(before),
        describe(after),
        limits_words(preview, after)
    )
}

fn say_the_limits(
    ctx: &Context,
    area: &str,
    config: &AreaName,
    shown: &Limits,
    preview: &LimitsPreview,
    changed: bool,
) -> Outcome {
    let json = serde_json::json!({ "area": area, "keep": shown.keep, "min_age_minutes": shown.min_age_seconds / 60, "cleanup": shown.cleanup, "would_remove": preview.would_remove, "changed": changed });
    if changed {
        ctx.show.result(
            format!(
                "{config} now {}. It takes effect at the next cleanup.\n",
                describe(shown)
            ),
            json,
        )
    } else {
        ctx.show.result(
            format!(
                "{config} {}.\n{}\n",
                describe(shown),
                limits_words(preview, shown)
            ),
            json,
        )
    }
}
