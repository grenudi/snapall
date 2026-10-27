// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What each command does, one function each. A function reads as the steps of its command in order;
//! the use cases (`snapall-core`) and the renderers (`render`) hold the details.

use std::error::Error;
use std::io::IsTerminal;
use std::process::ExitCode;

use snapall_core::boot;
use snapall_core::{
    all_snapshots, assess, config_covering, delete_snapshot, file_history, fresh_point, gather,
    plan_snapshots, preview_point, preview_restore, restore_point_along, restore_points,
    restore_snapshot, run_plan_along, snapshots_of, Act, ConfigName, Description, Level,
    MemberView, Point, Preview, RestoreError, RestorePoint, SnapshotBackend, SnapshotId,
};

use crate::output::{Outcome, Show};
use crate::{advice, ask, boot_probe, render, setup};

/// What every command works with: the backend (already observed, so every operation is reported),
/// how to show results, and whether tips are allowed.
pub struct Context<'a> {
    pub backend: &'a (dyn SnapshotBackend + Sync),
    pub show: &'a Show,
    pub hints: bool,
}

/// A change in progress: it holds the right to make a change (only one snapall does at a time, and a
/// second is told who has it and what for), and the journal knows of it from its start to its end, so
/// that the window can show what a command did. Reading never needs it.
struct Change {
    _hold: snapall_core::Hold,
    id: String,
    system: snapall_system::LinuxSystem,
    started: std::time::Instant,
    ended: bool,
}

impl Change {
    /// `now` is what is being done, for whoever is refused meanwhile; `done` is the same in the past
    /// tense, for the record.
    fn begin(now: &str, done: &str, name: Option<&str>) -> Result<Change, Box<dyn Error>> {
        use snapall_core::SystemProbe;
        let system = snapall_system::LinuxSystem::new();
        let hold = system.hold_changes(now).map_err(|busy| busy.to_string())?;
        let milliseconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis());
        let id = name.map_or_else(
            || format!("{}-{milliseconds}-c", std::process::id()),
            str::to_owned,
        );
        // a change that the window handed to the background says so; any other is a command
        let source = match std::env::var("SNAPALL_SOURCE").as_deref() {
            Ok("window") => snapall_core::Source::Window,
            _ => snapall_core::Source::Command,
        };
        system.record(&snapall_core::JournalEvent::Started {
            id: id.clone(),
            at: system.local_time().unwrap_or_default(),
            what: done.to_owned(),
            source,
            pid: std::process::id(),
        });
        Ok(Change {
            _hold: hold,
            id,
            system,
            started: std::time::Instant::now(),
            ended: false,
        })
    }

    /// How to get back from this change, worked out now, while the earlier state is known.
    fn rollback(&self, rollback: snapall_core::Rollback) {
        use snapall_core::SystemProbe;
        self.system.record(&snapall_core::JournalEvent::Rollback {
            id: self.id.clone(),
            at: self.system.local_time().unwrap_or_default(),
            rollback,
        });
    }

    /// How it went: what came of it, or why it failed.
    fn end(mut self, outcome: Result<String, String>) {
        self.write_end(outcome.into());
    }

    /// It was stopped on request, between steps; how far it had got.
    fn end_stopped(mut self, how_far: String) {
        self.write_end(snapall_core::JournalOutcome::Cancelled(how_far));
    }

    /// The steps it takes are reported as they begin, and a request to stop is looked for before each.
    fn course(&self) -> Following<'_> {
        Following { change: self }
    }

    fn write_end(&mut self, outcome: snapall_core::JournalOutcome) {
        use snapall_core::SystemProbe;
        self.ended = true;
        self.system.clear_stop(&self.id);
        self.system.record(&snapall_core::JournalEvent::Finished {
            id: self.id.clone(),
            at: self.system.local_time().unwrap_or_default(),
            outcome,
            took_ms: u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
        });
    }
}

impl Drop for Change {
    /// A command that leaves early, on an error, did not finish: the record says so rather than leaving
    /// a change that looks as if it were still running.
    fn drop(&mut self) {
        if !self.ended {
            self.write_end(snapall_core::JournalOutcome::Failed(
                "it ended early, with an error that the command printed".to_owned(),
            ));
        }
    }
}

/// What a change does between its steps: say how far it has come, and look for a request to stop.
struct Following<'a> {
    change: &'a Change,
}

impl snapall_core::Course for Following<'_> {
    fn proceed(&self, done: usize, of: usize, area: &str) -> bool {
        use snapall_core::SystemProbe;
        let system = &self.change.system;
        system.record(&snapall_core::JournalEvent::Progress {
            id: self.change.id.clone(),
            at: system.local_time().unwrap_or_default(),
            done: u32::try_from(done).unwrap_or(u32::MAX),
            of: u32::try_from(of).unwrap_or(u32::MAX),
            area: area.to_owned(),
        });
        !system.stop_requested(&self.change.id)
    }
}

// ---- reading ---------------------------------------------------------------------------------

pub fn list(ctx: &Context) -> Outcome {
    let configs = ctx.backend.list_configs()?;
    ctx.show
        .result(render::configs(&configs), render::configs_json(&configs))
}

pub fn snapshots(ctx: &Context, config: Option<ConfigName>) -> Outcome {
    let all = match config {
        Some(config) => {
            let snapshots = snapshots_of(ctx.backend, &config)?;
            vec![(config, snapshots)]
        }
        None => all_snapshots(ctx.backend)?,
    };
    ctx.show
        .result(render::snapshots(&all), render::snapshots_json(&all))
}

/// How one file stood in each snapshot.
pub fn history(ctx: &Context, path: &str) -> Outcome {
    let path = absolute(path)?;
    let found = file_history(ctx.backend, &path).map_err(|e| advice::explain(&e, &path))?;
    ctx.show
        .result(render::history(&found), render::history_json(&found))
}

/// What restoring a snapshot would change, without changing it.
pub fn changes(ctx: &Context, config: &ConfigName, id: SnapshotId, path: Option<&str>) -> Outcome {
    let preview = preview_restore(ctx.backend, config, id, path)?;
    show_changes(ctx, config, id, &preview)
}

// ---- changing --------------------------------------------------------------------------------

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

pub fn delete(ctx: &Context, config: &ConfigName, id: SnapshotId, destroy: bool) -> Outcome {
    // What is about to be lost is said first. A snapshot that is not there is not asked about: the attempt
    // fails and says so, and the failed attempt is in the audit trail, as it always was.
    if let Some(snapshot) = ctx
        .backend
        .list_snapshots(config)?
        .into_iter()
        .find(|snapshot| snapshot.id == id)
    {
        let what = format!(
            "snapshot #{id} of {config}{}{}",
            snapshot
                .date
                .as_deref()
                .map_or(String::new(), |date| format!(" from {date}")),
            if snapshot.description.is_empty() {
                String::new()
            } else {
                format!(" (\"{}\")", snapshot.description)
            }
        );
        eprintln!("{what} will be deleted for good. Only this saved moment goes: the files in {config} are not touched.\n");
        if let Some(stop) = ask::gate(
            Act::DeleteSnapshots.weight(),
            destroy,
            &format!("Delete {what}?"),
            None,
        )? {
            return Ok(stop);
        }
    }
    let change = Change::begin(
        &format!("Deleting 1 snapshot of {config}"),
        &format!("Deleted 1 snapshot of {config}"),
        None,
    )?;
    let result = delete_snapshot(ctx.backend, config, id);
    if result.is_ok() {
        change.rollback(snapall_core::Rollback::Advice(
            snapall_core::explain::DELETE_ADVICE.to_owned(),
        ));
    }
    change.end(
        result
            .as_ref()
            .map(|()| String::new())
            .map_err(ToString::to_string),
    );
    result?;
    ctx.show.result(
        render::deleted(config, id),
        render::deleted_json(config, id),
    )
}

/// Put a config, or one path of it, back to how a snapshot had it. It shows what will change, asks
/// first (unless told not to), and the use case takes a snapshot of the current state before it
/// touches anything.
pub fn restore(
    ctx: &Context,
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Outcome {
    let preview = preview_restore(ctx.backend, config, id, path)?;
    refuse_to_restore_the_whole_system_root(&preview, config, path)?;
    if dry_run || preview.changes.is_empty() {
        return show_changes(ctx, config, id, &preview);
    }
    match confirmation(yes, config, id, &preview)? {
        Confirmation::Given => {}
        Confirmation::NoTerminalToAskOn => return Ok(ExitCode::from(2)),
        Confirmation::Declined => return Ok(ExitCode::FAILURE),
    }
    let (now, past) = match path {
        Some(path) => (
            format!("Restoring {path} from snapshot #{id} of {config}"),
            format!("Restored {path} from snapshot #{id} of {config}"),
        ),
        None => (
            format!("Putting {config} back to snapshot #{id}"),
            format!("Put {config} back to snapshot #{id}"),
        ),
    };
    let change = Change::begin(&now, &past, None)?;
    let result = restore_snapshot(ctx.backend, config, id, path);
    if let Ok(done) = &result {
        change.rollback(snapall_core::rollback::after_restoring(
            config,
            path,
            done.safety_snapshot,
        ));
    }
    change.end(
        result
            .as_ref()
            .map(|_| String::new())
            .map_err(ToString::to_string),
    );
    let done = result?;
    ctx.show.result(
        render::restored(config, id, &done),
        render::restored_json(config, id, &done),
    )
}

/// How protection stands: whether it is working, the room it takes, whether old snapshots are cleaned
/// up, and what to do about anything that is not. It exits with failure only for what needs doing now.
pub fn health(ctx: &Context) -> Outcome {
    let facts = gather(ctx.backend, &snapall_system::LinuxSystem::new())?;
    let report = assess(&facts);
    ctx.show.result(
        render::health(&facts, &report),
        render::health_json(&facts, &report),
    )?;
    Ok(if report.verdict.level == Level::Problem {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Ask a running change to stop after the area it is on. Only a request: the change looks for it between
/// its areas, never inside one, so nothing is left half-done.
pub fn cancel(ctx: &Context, job: &str, yes: bool) -> Outcome {
    use snapall_core::{JournalStatus, SystemProbe};
    let system = snapall_system::LinuxSystem::new();
    let history = system.history(500);
    let Some(entry) = history.iter().find(|entry| entry.id == job) else {
        return Err(format!(
            "there is no change called `{job}`: `snapall activity --json` lists them"
        )
        .into());
    };
    if entry.status != JournalStatus::Running {
        return Err(format!(
            "`{}` has already ended ({})",
            entry.what,
            entry.status.word().to_lowercase()
        )
        .into());
    }
    let question = format!("Ask {} to stop after the area it is on?", entry.what);
    if let Some(stop) = ask::gate(Act::StopChange.weight(), yes, &question, None)? {
        return Ok(stop);
    }
    system.request_stop(job)?;
    ctx.show.result(
        format!(
            "Asked {} to stop. It finishes the area it is on, then stops; nothing is left half-done.\n",
            entry.what
        ),
        serde_json::json!({ "asked": job, "what": entry.what }),
    )
}

/// What snapall has done, newest first: from the window and from commands, in this and earlier sessions.
pub fn activity(ctx: &Context) -> Outcome {
    use snapall_core::SystemProbe;
    let entries = snapall_system::LinuxSystem::new().history(100);
    ctx.show
        .result(render::activity(&entries), render::activity_json(&entries))
}

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
    if let Some(stop) = ask::gate(
        act.weight(),
        if removes { destroy } else { yes },
        "Apply this?",
        None,
    )? {
        return Ok(stop);
    }
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

/// The areas of a group, out of the ones protected now; a group that names one that is not is refused.
fn areas_of_group(ctx: &Context, wanted: &str) -> Result<Vec<ConfigName>, Box<dyn Error>> {
    use snapall_core::{SystemProbe, Wanted};
    let wanted = Wanted::parse(wanted)?;
    let groups = snapall_system::LinuxSystem::new().groups()?;
    let protected = ctx.backend.list_configs()?;
    Ok(groups.resolve(&wanted, &protected)?)
}

/// Sets of areas that have a name. Without an action they are listed; `add` makes or replaces one and
/// `remove` takes one away. `all` is always there, is every protected area, and cannot be changed.
pub fn groups(
    ctx: &Context,
    action: Option<&str>,
    name: Option<&str>,
    areas: Option<&str>,
    yes: bool,
) -> Outcome {
    use snapall_core::{GroupName, SystemProbe};
    let system = snapall_system::LinuxSystem::new();
    let mut made = system.groups()?;
    let protected = ctx.backend.list_configs()?;
    match action {
        None | Some("list") => ctx.show.result(
            render::groups(&made, &protected),
            render::groups_json(&made, &protected),
        ),
        Some("add") => {
            let name =
                GroupName::parse(name.ok_or("which group? `snapall groups add NAME AREA,AREA`")?)?;
            let wanted = areas.ok_or("which areas? `snapall groups add NAME AREA,AREA`")?;
            let list = crate::cli::parse_only(wanted)?;
            if let Some(unprotected) = list.iter().find(|area| !protected.contains(area)) {
                let there: Vec<String> = protected.iter().map(ToString::to_string).collect();
                return Err(format!(
                    "`{unprotected}` is not a protected area (there are: {})",
                    there.join(", ")
                )
                .into());
            }
            let before = made.get(&name).map(<[ConfigName]>::to_vec);
            let question = format!(
                "Make the group {name} of {}?",
                list.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if let Some(stop) = ask::gate(Act::ChangeGroups.weight(), yes, &question, None)? {
                return Ok(stop);
            }
            let change = Change::begin(
                &format!("Making the group {name}"),
                &format!("Made the group {name}"),
                None,
            )?;
            let saved = made
                .set(name.clone(), list)
                .map_err(|e| e.to_string())
                .and_then(|()| system.save_groups(&made));
            if saved.is_ok() {
                change.rollback(snapall_core::rollback::after_groups(
                    &name,
                    before.as_deref(),
                ));
            }
            change.end(saved.clone().map(|()| String::new()));
            saved?;
            ctx.show.result(
                format!(
                    "The group {name} is now: {}
",
                    made.get(&name).map_or(String::new(), |a| a
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "))
                ),
                serde_json::json!({ "group": name.to_string(), "changed": true }),
            )
        }
        Some("remove") => {
            let name = GroupName::parse(name.ok_or("which group? `snapall groups remove NAME`")?)?;
            let before = made.get(&name).map(<[ConfigName]>::to_vec);
            if before.is_none() {
                return Err(format!("there is no group called `{name}`").into());
            }
            let question =
                format!("Take the group {name} away? The areas and their snapshots are untouched.");
            if let Some(stop) = ask::gate(Act::ChangeGroups.weight(), yes, &question, None)? {
                return Ok(stop);
            }
            made.remove(&name);
            let change = Change::begin(
                &format!("Removing the group {name}"),
                &format!("Removed the group {name}"),
                None,
            )?;
            let saved = system.save_groups(&made);
            if saved.is_ok() {
                change.rollback(snapall_core::rollback::after_groups(
                    &name,
                    before.as_deref(),
                ));
            }
            change.end(saved.clone().map(|()| String::new()));
            saved?;
            ctx.show.result(
                format!(
                    "The group {name} is gone. The areas and their snapshots are untouched.
"
                ),
                serde_json::json!({ "group": name.to_string(), "changed": true }),
            )
        }
        Some(other) => Err(format!("`{other}` is not `add` or `remove`").into()),
    }
}

/// Whether what a rollback would go back to is still there. It is checked when the rollback is asked for, not
/// when it was recorded: snapper's cleanup may have removed the snapshot since, and then the honest answer is
/// that it cannot be done.
fn check_the_way_back_still_exists(
    ctx: &Context,
    commands: &[Vec<String>],
) -> Result<(), Box<dyn Error>> {
    for command in commands {
        match command
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["restore", config, id, ..] => {
                let config = ConfigName::parse(config)?;
                let number: u64 = id
                    .parse()
                    .map_err(|_| format!("`{id}` is not a snapshot number"))?;
                let there = ctx
                    .backend
                    .list_snapshots(&config)?
                    .iter()
                    .any(|snapshot| snapshot.id.get() == number);
                if !there {
                    return Err(format!(
                        "snapshot #{number} of {config}, which this would go back to, no longer exists: cleanup has removed it, so this cannot be rolled back"
                    )
                    .into());
                }
            }
            ["restore-all", point, ..] => {
                let there = restore_points(&all_snapshots(ctx.backend)?)
                    .iter()
                    .any(|candidate| candidate.point.to_string() == *point);
                if !there {
                    return Err(format!(
                        "restore point {point}, which this would go back to, no longer exists: cleanup has removed it, so this cannot be rolled back"
                    )
                    .into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Roll a change back to the state it recorded, if it recorded one. It says what that state is and what it
/// will run, asks as heavily as what it does weighs (undoing the taking of a snapshot is deleting it), and
/// runs the recorded commands through this same program, so they are checked and journaled like any other.
pub fn undo(ctx: &Context, change_id: &str, yes: bool, destroy: bool, no_dbus: bool) -> Outcome {
    use snapall_core::{Rollback, SystemProbe, Weight};
    let system = snapall_system::LinuxSystem::new();
    let history = system.history(500);
    let Some(entry) = history.iter().find(|entry| entry.id == change_id) else {
        return Err(format!(
            "there is no change called `{change_id}`: `snapall activity` lists them"
        )
        .into());
    };
    let undo = match &entry.rollback {
        Some(Rollback::Undo(undo)) => undo,
        Some(Rollback::Advice(text)) => {
            eprintln!("{} cannot be undone.\n{text}", entry.what);
            return Ok(ExitCode::FAILURE);
        }
        None => {
            return Err(format!(
                "no way back was recorded for `{}`: changes made before snapall recorded one have none",
                entry.what
            )
            .into());
        }
    };
    check_the_way_back_still_exists(ctx, &undo.commands)?;
    eprintln!("To roll back: {}\n  {}\n", entry.what, undo.what);
    for command in &undo.commands {
        eprintln!("  snapall {}", command.join(" "));
    }
    eprintln!();
    let said_so = if undo.weight == Weight::Destroys {
        destroy
    } else {
        yes
    };
    let act = Act::Undo { doing: undo.weight };
    if let Some(stop) = ask::gate(act.weight(), said_so, "Roll this back?", None)? {
        return Ok(stop);
    }
    let program = std::env::current_exe()?;
    for command in &undo.commands {
        let mut step = std::process::Command::new(&program);
        if no_dbus {
            step.arg("--no-dbus");
        }
        step.args(command);
        // it has been answered here, at the weight of what it does
        if let Some(flag) = undo.weight.script_flag() {
            step.arg(flag);
        }
        if !step.status()?.success() {
            return Err(format!(
                "`snapall {}` did not succeed, so the rollback stopped there and nothing after it was done",
                command.join(" ")
            )
            .into());
        }
    }
    ctx.show.result(
        format!("Rolled back: {}\n", undo.what),
        serde_json::json!({ "change": change_id, "rolled_back": true }),
    )
}

/// How the disk counts the room snapshots take, and, given `on` or `off`, switching it. Switching changes
/// how the whole disk works, so it explains what it gives and costs and asks first.
pub fn quotas(ctx: &Context, wanted: Option<&str>, yes: bool) -> Outcome {
    use snapall_core::explain::room_counting_text;
    use snapall_core::{QuotaMode, SystemProbe};
    let system = snapall_system::LinuxSystem::new();
    let facts = gather(ctx.backend, &system)?;
    let Some(first) = facts.areas.first() else {
        return Err(
            "nothing is protected yet, so there is nothing to count: run `snapall setup`".into(),
        );
    };
    let mode = first.quotas;
    let word = match mode {
        QuotaMode::Off => "off",
        QuotaMode::Full => "on",
        QuotaMode::Simple => "on, the simple kind",
        QuotaMode::Unknown => "could not be found out",
    };
    let Some(wanted) = wanted else {
        return ctx.show.result(
            format!(
                "Counting the room snapshots take: {word}\n\n{}\n",
                room_counting_text(mode)
            ),
            serde_json::json!({ "counting": word, "explanation": room_counting_text(mode) }),
        );
    };
    let turn_on = match wanted {
        "on" => true,
        "off" => false,
        other => return Err(format!("`{other}` is not `on` or `off`").into()),
    };
    if mode == QuotaMode::Simple && turn_on {
        return Err("the disk counts room the simple way, and snapall cannot switch it to the full kind in place".into());
    }
    if (turn_on && mode == QuotaMode::Full) || (!turn_on && mode == QuotaMode::Off) {
        return ctx.show.result(
            format!("Counting the room snapshots take is already {word}.\n"),
            serde_json::json!({ "counting": word, "changed": false }),
        );
    }
    let areas: Vec<String> = facts
        .areas
        .iter()
        .map(snapall_core::AreaFacts::title)
        .collect();
    if !yes && ask::can_ask() {
        eprintln!("{}\n", room_counting_text(mode));
    }
    let question = format!(
        "Switch counting {} for the disk{} holding {}?",
        if turn_on { "on" } else { "off" },
        if areas.len() == 1 { "" } else { "s" },
        areas.join(", ")
    );
    if let Some(stop) = ask::gate(Act::RoomCounting.weight(), yes, &question, None)? {
        return Ok(stop);
    }
    let side = if turn_on { "on" } else { "off" };
    let change = Change::begin(
        &format!("Switching room counting {side}"),
        &format!("Switched room counting {side}"),
        None,
    )?;
    let result: Result<(), String> = facts
        .areas
        .iter()
        .try_for_each(|area| system.set_quotas(&area.subvolume, turn_on));
    if result.is_ok() {
        change.rollback(snapall_core::rollback::after_room_counting(turn_on));
    }
    change.end(result.clone().map(|()| String::new()));
    result?;
    ctx.show.result(
        format!(
            "Counting the room snapshots take is now {}.{}\n",
            if turn_on { "on" } else { "off" },
            if turn_on {
                " The disk builds its tally in the background; figures appear as it finishes."
            } else {
                ""
            }
        ),
        serde_json::json!({ "counting": if turn_on { "on" } else { "off" }, "changed": true }),
    )
}

/// The restore points: the snapshots that one action took together.
pub fn points(ctx: &Context) -> Outcome {
    let found = restore_points(&all_snapshots(ctx.backend)?);
    ctx.show
        .result(render::points(&found), render::points_json(&found))
}

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

fn the_point_named<'a>(
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
fn point_confirmation(
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

fn show_changes(ctx: &Context, config: &ConfigName, id: SnapshotId, preview: &Preview) -> Outcome {
    ctx.show.result(
        render::changes(config, id, preview),
        render::changes_json(config, id, preview),
    )
}

/// Restoring all of the system root while it runs would leave files, kernel and boot menu out of
/// step: a deliberate refusal (not a usage error, not a crash), with the way that works instead.
fn refuse_to_restore_the_whole_system_root(
    preview: &Preview,
    config: &ConfigName,
    path: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if preview.is_system_root && path.is_none() {
        let route = boot::restore_route(&boot::assess(&boot_probe::system()));
        let refusal = RestoreError::SystemRoot {
            config: config.clone(),
        };
        return Err(format!("{refusal}\n{route}").into());
    }
    Ok(())
}

enum Confirmation {
    Given,
    /// Nobody to ask: the caller must say `--yes`, so nothing is changed.
    NoTerminalToAskOn,
    Declined,
}

/// Restoring changes files, so the person is asked, unless they said `--yes`.
fn confirmation(
    yes: bool,
    config: &ConfigName,
    id: SnapshotId,
    preview: &Preview,
) -> Result<Confirmation, Box<dyn Error>> {
    if yes {
        return Ok(Confirmation::Given);
    }
    if !ask::can_ask() {
        eprintln!(
            "snapall: restore changes files and there is no terminal to ask on; \
             look first with `snapall changes`, then pass --yes"
        );
        return Ok(Confirmation::NoTerminalToAskOn);
    }
    eprint!("{}", render::changes(config, id, preview));
    eprintln!("\n{}\n", snapall_core::explain::RESTORE_CAUTION);
    let count = preview.changes.len();
    let paths = if count == 1 { "path" } else { "paths" };
    let question = format!(
        "Restore {count} {paths} of {config} to #{id}? The current state is snapshotted first, so this can be undone."
    );
    if ask::yes_no(&question)? {
        Ok(Confirmation::Given)
    } else {
        eprintln!("snapall: not restoring");
        Ok(Confirmation::Declined)
    }
}

/// After a snapshot: a nudge to a person, on stderr (stdout stays parseable), and only when a
/// boot-menu tool would help.
fn suggest_the_boot_menu(ctx: &Context) {
    let to_a_person = std::io::stderr().is_terminal() && !ctx.show.is_json();
    if !ctx.hints || !to_a_person {
        return;
    }
    if let Some(tip) = boot::hint(&boot::assess(&boot_probe::system())) {
        eprintln!("{tip}");
    }
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

// ---- the terminal UI -------------------------------------------------------------------------

/// Open the terminal UI, on every config or on one file.
pub fn tui(ctx: &Context, file: Option<&str>) -> Outcome {
    require_a_terminal()?;
    // Which config covers the file is settled here, before the terminal is taken over, so that a
    // file nothing covers is an error message and not a screen that opens empty.
    let target = file
        .map(|named| file_target(ctx.backend, named))
        .transpose()?;
    let context = ui_context(ctx, target);
    match snapall_tui::run(ctx.backend, &snapall_system::LinuxSystem::new(), context)? {
        snapall_tui::Exit::Quit => Ok(ExitCode::SUCCESS),
        // Exit as a signalled process would, once the terminal is back to normal.
        snapall_tui::Exit::Signal(signal) => {
            Ok(ExitCode::from(u8::try_from(128 + signal).unwrap_or(1)))
        }
    }
}

fn require_a_terminal() -> Result<(), Box<dyn Error>> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Ok(())
    } else {
        Err("`snapall tui` needs an interactive terminal".into())
    }
}

/// The file the UI opens on, and the config whose snapshots hold its history.
fn file_target(
    backend: &dyn SnapshotBackend,
    named: &str,
) -> Result<snapall_tui::FileTarget, Box<dyn Error>> {
    let path = absolute(named)?;
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
        return Err("directories are not supported yet: choose a file".into());
    }
    let config = config_covering(backend, &path).map_err(|e| advice::explain(&e, &path))?;
    Ok(snapall_tui::FileTarget { path, config })
}

/// What the UI is told about this machine. It is looked at once, here, because the UI's model must
/// not read the machine itself.
fn ui_context(ctx: &Context, file: Option<snapall_tui::FileTarget>) -> snapall_tui::UiContext {
    let boot_menu = boot::assess(&boot_probe::system());
    // What could be protected matters to the whole window, not to one file's history.
    let facts = setup::gather();
    let offers = if file.is_some() {
        Vec::new()
    } else {
        setup::offers(ctx.backend, &facts)
    };
    snapall_tui::UiContext {
        boot_tip: if ctx.hints {
            boot::hint(&boot_menu)
        } else {
            None
        },
        restore_route: boot::restore_route(&boot_menu),
        file,
        protect_hint: setup::hint_for(&offers),
        offers,
        administrator: facts.is_root,
    }
}

// ---- helpers ---------------------------------------------------------------------------------

/// The absolute path of what the user named. A relative path is taken from the working directory,
/// and symlinks in the *directory* part are resolved, so that a path reached through `/home` being
/// a link to `/var/home` is found in the subvolume that really holds it. The file's own name is
/// kept as given: a symlink is itself what is asked about, and a deleted file has nothing to resolve.
fn absolute(path: &str) -> Result<String, Box<dyn Error>> {
    let given = std::path::Path::new(path);
    let joined = if given.is_absolute() {
        given.to_owned()
    } else {
        std::env::current_dir()?.join(given)
    };
    let (Some(parent), Some(name)) = (joined.parent(), joined.file_name()) else {
        return Err(format!("`{path}` does not name a file").into());
    };
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("{}: {e}", parent.display()))?;
    parent
        .join(name)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| "a path must be valid UTF-8".into())
}

/// Seconds since 1970, which is what names a restore point after the moment an action started.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
