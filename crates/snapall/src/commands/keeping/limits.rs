// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How many snapshots an area keeps, and when its oldest go.

use super::*;
use crate::ask;
use crate::output::Outcome;
use snapall_core::{Act, ConfigName};
use std::error::Error;

/// What was asked for, as typed: each may be absent.
pub struct LimitsWanted<'a> {
    pub keep: Option<&'a str>,
    pub min_age: Option<&'a str>,
    pub cleanup: Option<&'a str>,
}

/// How many snapshots an area keeps, and when its oldest go. Without options it shows them and what the
/// next cleanup would remove; with options it changes them, after saying what that would remove (a removed
/// snapshot cannot be brought back, so a change that removes any asks first, or needs `--yes`).
pub fn limits(
    ctx: &Context,
    area: &str,
    wanted: LimitsWanted<'_>,
    yes: bool,
    destroy: bool,
) -> Outcome {
    use snapall_core::{limits_words, preview_limits, Limits, SystemProbe};
    let config = ConfigName::parse(area)?;
    if !ctx.backend.list_configs()?.contains(&config) {
        return Err(format!("`{area}` is not a protected area: `snapall list` shows them").into());
    }
    let before = Limits::from_retention(&ctx.backend.retention(&config)?).ok_or(
        "snapper did not say how many this area keeps, so there is nothing to show or change",
    )?;
    let number = |text: &str, what: &str| -> Result<u64, Box<dyn Error>> {
        text.trim()
            .parse::<u64>()
            .map_err(|_| format!("{what} must be a whole number, not `{text}`").into())
    };
    let mut after = before;
    if let Some(text) = wanted.keep {
        after.keep = u32::try_from(number(text, "--keep")?).map_err(|_| "--keep is too large")?;
    }
    if let Some(text) = wanted.min_age {
        after.min_age_seconds = number(text, "--min-age")? * 60;
    }
    match wanted.cleanup {
        Some("on") => after.cleanup = true,
        Some("off") => after.cleanup = false,
        Some(other) => return Err(format!("--cleanup must be `on` or `off`, not `{other}`").into()),
        None => {}
    }
    if let Some(problem) = after.problem() {
        return Err(problem.into());
    }
    let system = snapall_system::LinuxSystem::new();
    let now = system
        .local_time()
        .as_deref()
        .and_then(snapall_core::calendar::seconds_from_snapper_date);
    let snapshots = ctx.backend.list_snapshots(&config)?;
    let preview = preview_limits(&snapshots, &after, now);
    let describe = |l: &Limits| {
        format!(
            "keeps the newest {}, never removes one younger than {} minutes, cleanup {}",
            l.keep,
            l.min_age_seconds / 60,
            if l.cleanup { "on" } else { "off" }
        )
    };
    let options = after.changes_from(&before);
    if options.is_empty() {
        return ctx.show.result(
            format!("{config} {}.
{}
", describe(&before), limits_words(&preview, &before)),
            serde_json::json!({ "area": area, "keep": before.keep, "min_age_minutes": before.min_age_seconds / 60, "cleanup": before.cleanup, "would_remove": preview.would_remove, "changed": false }),
        );
    }
    eprintln!(
        "{config} currently {}.
After: {}.
{}
",
        describe(&before),
        describe(&after),
        limits_words(&preview, &after)
    );
    // a change that would remove snapshots is destroying; any other is a change like the rest
    let removes = preview.would_remove > 0;
    let act = Act::ChangeLimits { removes };
    ask::gate(
        act.weight(),
        if removes { destroy } else { yes },
        "Apply this?",
        None,
    )?;
    let change = Change::begin(
        &format!("Changing how many snapshots {config} keeps"),
        &format!("Changed how many snapshots {config} keeps"),
        None,
    )?;
    let result: Result<(), String> = options.iter().try_for_each(|option| {
        ctx.backend
            .set_config_option(&config, *option)
            .map_err(|e| e.to_string())
    });
    if result.is_ok() {
        change.rollback(snapall_core::rollback::after_limits(
            &config.to_string(),
            &before,
        ));
    }
    change.end(result.clone().map(|()| String::new()));
    result?;
    ctx.show.result(
        format!("{config} now {}. It takes effect at the next cleanup.
", describe(&after)),
        serde_json::json!({ "area": area, "keep": after.keep, "min_age_minutes": after.min_age_seconds / 60, "cleanup": after.cleanup, "would_remove": preview.would_remove, "changed": true }),
    )
}
