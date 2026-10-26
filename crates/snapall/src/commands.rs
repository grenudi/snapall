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
    restore_snapshot, run_plan_along, snapshots_of, ConfigName, Description, Level, MemberView,
    Point, Preview, RestoreError, RestorePoint, SnapshotBackend, SnapshotId,
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
    description: &Description,
    dry_run: bool,
    job: Option<&str>,
) -> Outcome {
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

pub fn delete(ctx: &Context, config: &ConfigName, id: SnapshotId) -> Outcome {
    let change = Change::begin(
        &format!("Deleting 1 snapshot of {config}"),
        &format!("Deleted 1 snapshot of {config}"),
        None,
    )?;
    let result = delete_snapshot(ctx.backend, config, id);
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
pub fn cancel(ctx: &Context, job: &str) -> Outcome {
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
    if !yes {
        if !ask::can_ask() {
            eprintln!("snapall: this changes how the whole disk works and there is no terminal to ask on; read `snapall quotas`, then pass --yes");
            return Ok(ExitCode::from(2));
        }
        eprintln!("{}\n", room_counting_text(mode));
        let question = format!(
            "Switch counting {} for the disk{} holding {}?",
            if turn_on { "on" } else { "off" },
            if areas.len() == 1 { "" } else { "s" },
            areas.join(", ")
        );
        if !ask::yes_no(&question)? {
            eprintln!("snapall: nothing changed");
            return Ok(ExitCode::FAILURE);
        }
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
