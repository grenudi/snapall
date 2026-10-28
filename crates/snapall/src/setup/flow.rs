// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The setup conversation: find what is not protected, explain what that means, ask, and set it up.

use super::*;
use snapall_core::protecting::coverage::{self, Area};
use snapall_core::vocabulary::explain::{self, Cleanup};
use snapall_core::{protect_area, SnapshotBackend};
use std::error::Error;

pub fn run(
    backend: &dyn SnapshotBackend,
    facts: &Facts,
    talk: &mut dyn Talk,
    options: &Options,
) -> Result<Report, Box<dyn Error>> {
    require_administrator_rights(facts, options)?;
    let areas = find_the_areas(backend, facts)?;
    let mut report = Report {
        areas: areas.iter().map(area_report).collect(),
        protected_now: Vec::new(),
        declined: Vec::new(),
        failed: Vec::new(),
        cleanup: facts.cleanup,
        cleanup_switched_on: false,
        dry_run: options.dry_run,
    };

    explain_the_basics(talk);
    list_the_areas(talk, &areas);
    let to_protect = choose_what_to_protect(&areas, options)?;
    if to_protect.is_empty() {
        say_nothing_needs_doing(talk, &areas, options);
    }
    for area in to_protect {
        offer_protection(backend, talk, options, &area, &mut report);
    }
    offer_cleanup(talk, options, facts, &limits_of(backend), &mut report);
    say_whats_next(talk, options, &report);
    Ok(report)
}

pub(super) fn require_administrator_rights(
    facts: &Facts,
    options: &Options,
) -> Result<(), Box<dyn Error>> {
    if facts.is_root || options.dry_run {
        return Ok(());
    }
    Err("Setting up protection changes how your computer keeps snapshots, which needs administrator rights. Run it again as:\n\n    sudo snapall setup\n\n(Add --dry-run to only look at what it would do.)".into())
}

pub(super) fn find_the_areas(
    backend: &dyn SnapshotBackend,
    facts: &Facts,
) -> Result<Vec<Area>, Box<dyn Error>> {
    let mut configs = Vec::new();
    for name in backend.list_areas()? {
        let location = backend.area_location(&name)?;
        configs.push((name, location));
    }
    let mounts = coverage::mounts_from_mountinfo(&facts.mountinfo);
    Ok(coverage::areas(&mounts, &configs))
}

pub(super) fn area_report(area: &Area) -> AreaReport {
    let info = coverage::describe(&area.mount_point);
    AreaReport {
        mount_point: area.mount_point.clone(),
        title: info.title,
        protected_by: area.protected_by.clone(),
        worth: info.worth,
    }
}

/// What each area keeps and protects from cleanup: its real settings, read from snapper.
pub(super) fn limits_of(backend: &dyn SnapshotBackend) -> Vec<explain::Limit> {
    let Ok(configs) = backend.list_areas() else {
        return Vec::new();
    };
    configs
        .iter()
        .filter_map(|config| {
            let location = backend.area_location(config).ok()?;
            let retention = backend.retention(config).ok()?;
            Some(explain::Limit {
                area: coverage::describe(&location).title,
                keep: retention.keep,
                min_age_seconds: retention.min_age_seconds,
            })
        })
        .collect()
}

pub(super) fn explain_the_basics(talk: &mut dyn Talk) {
    talk.say(&format!(
        "{}\n\n{}\n\n{}\n",
        explain::WHAT_IS_A_SNAPSHOT,
        explain::WHAT_IS_AN_AREA,
        explain::NOT_A_BACKUP
    ));
}

pub(super) fn list_the_areas(talk: &mut dyn Talk, areas: &[Area]) {
    if areas.is_empty() {
        talk.say("snapall did not find a part of this computer's disk that it knows how to protect. It works with disks formatted as Btrfs, and none seems to be in use here.\n");
        return;
    }
    let mut text =
        String::from("The parts of your disk, and whether snapshots are kept for them:\n");
    for area in areas {
        text.push_str(&format!(
            "\n  {}\n      {}\n",
            explain::area_line(area),
            explain::area_detail(&coverage::describe(&area.mount_point))
        ));
    }
    talk.say(&text);
}

/// The areas to offer protection to: the one the person named, or those worth protecting that are not.
pub(super) fn choose_what_to_protect(
    areas: &[Area],
    options: &Options,
) -> Result<Vec<Area>, Box<dyn Error>> {
    let Some(named) = &options.area else {
        return Ok(coverage::gaps(areas).into_iter().cloned().collect());
    };
    let wanted = named.trim_end_matches('/');
    let wanted = if wanted.is_empty() { "/" } else { wanted };
    match areas.iter().find(|a| {
        a.mount_point.trim_end_matches('/') == wanted.trim_end_matches('/')
            || a.mount_point == wanted
    }) {
        Some(area) if area.is_protected() => Ok(Vec::new()),
        Some(area) => Ok(vec![area.clone()]),
        None => {
            let known: Vec<&str> = areas.iter().map(|a| a.mount_point.as_str()).collect();
            Err(format!(
                "{named} is not a part of the disk that snapall can protect. The parts it found are: {}.",
                known.join(", ")
            )
            .into())
        }
    }
}

/// There is nothing to set up. Saying so is part of the answer: silence would leave the person
/// wondering whether anything happened.
pub(super) fn say_nothing_needs_doing(talk: &mut dyn Talk, areas: &[Area], options: &Options) {
    let message = match &options.area {
        Some(named) => {
            let title = coverage::describe(named).title;
            format!("{title} ({named}) is already protected, so there is nothing to set up.\n")
        }
        None if areas.is_empty() => return,
        None => "Everything snapall recommends protecting is protected already, so there is nothing to set up.\n".to_owned(),
    };
    talk.say(&message);
}

pub(super) fn offer_protection(
    backend: &dyn SnapshotBackend,
    talk: &mut dyn Talk,
    options: &Options,
    area: &Area,
    report: &mut Report,
) {
    let title = coverage::describe(&area.mount_point).title;
    talk.say(&format!("{}\n", explain::protect_offer(&area.mount_point)));
    if options.dry_run {
        talk.say("(Dry run: nothing was changed.)\n");
        return;
    }
    let go = options.yes || matches!(talk.ask(&format!("Protect {title} now?")), Ok(true));
    if !go {
        talk.say(&format!(
            "Skipped. {title} stays unprotected; run `{}` whenever you want to.\n",
            setup_command(options)
        ));
        report.declined.push(area.mount_point.clone());
        return;
    }
    match protect_area(backend, &area.mount_point) {
        Ok(name) => {
            talk.say(&format!("{}\n", explain::protected_text(&area.mount_point)));
            report.protected_now.push((area.mount_point.clone(), name));
        }
        Err(error) => {
            let why = explain::failure_text(&title, &error);
            talk.say(&format!("{why}\n"));
            report.failed.push((area.mount_point.clone(), why));
        }
    }
}

pub(super) fn offer_cleanup(
    talk: &mut dyn Talk,
    options: &Options,
    facts: &Facts,
    limits: &[explain::Limit],
    report: &mut Report,
) {
    let state = facts.cleanup;
    talk.say(&format!(
        "{}\n",
        explain::cleanup_text(state, &facts.cleanup_schedule, limits)
    ));
    if state != Cleanup::Off {
        return;
    }
    talk.say(&format!(
        "{}\n",
        explain::cleanup_offer(&facts.cleanup_schedule, limits)
    ));
    if options.dry_run {
        return;
    }
    if !(options.yes || matches!(talk.ask("Switch on automatic cleanup now?"), Ok(true))) {
        return;
    }
    match talk.switch_on_cleanup() {
        Ok(()) => {
            talk.say("Automatic cleanup is on.\n");
            report.cleanup_switched_on = true;
        }
        Err(error) => {
            let why =
                format!("Could not switch automatic cleanup on: {error}. Nothing was changed.");
            talk.say(&format!("{why}\n"));
            report.failed.push(("cleanup".into(), why));
        }
    }
}

/// The command to run this again, as the person ran it.
pub(super) fn setup_command(options: &Options) -> String {
    match &options.area {
        Some(area) => format!("sudo snapall setup {area}"),
        None => "sudo snapall setup".to_owned(),
    }
}

pub(super) fn say_whats_next(talk: &mut dyn Talk, options: &Options, report: &Report) {
    let mut next = String::from("What next:\n");
    if report.dry_run {
        next.push_str(&format!(
            "  - Run `{}` to do this.\n",
            setup_command(options)
        ));
    }
    if !report.protected_now.is_empty() {
        next.push_str("  - Take the first snapshot of everything with `sudo snapall`.\n");
    }
    next.push_str("  - Open the snapall window with `sudo snapall tui`: it lists your snapshots and restores from them.\n");
    next.push_str(
        "  - Or right-click a file in your file manager and choose \"Restore from snapshots\".\n",
    );
    talk.say(&next);
}
