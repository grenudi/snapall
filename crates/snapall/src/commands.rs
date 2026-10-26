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
    plan_snapshots, preview_point, preview_restore, restore_point, restore_points,
    restore_snapshot, run_plan, snapshots_of, ConfigName, Description, Level, MemberView, Point,
    Preview, RestoreError, RestorePoint, SnapshotBackend, SnapshotId,
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
) -> Outcome {
    let plan = plan_snapshots(ctx.backend, only)?;
    if dry_run {
        return ctx.show.result(
            render::dry_run(&plan, description),
            render::dry_run_json(&plan, description),
        );
    }
    // every snapshot of this one action is the same restore point
    let point = fresh_point(ctx.backend, unix_now());
    let report = run_plan(ctx.backend, &plan, description, Some(&point));
    ctx.show
        .result(render::report(&report), render::report_json(&report))?;
    if !report.is_success() {
        return Ok(ExitCode::FAILURE);
    }
    suggest_the_boot_menu(ctx);
    Ok(ExitCode::SUCCESS)
}

pub fn delete(ctx: &Context, config: &ConfigName, id: SnapshotId) -> Outcome {
    delete_snapshot(ctx.backend, config, id)?;
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
    let done = restore_snapshot(ctx.backend, config, id, path)?;
    ctx.show.result(
        render::restored(config, id, &done),
        render::restored_json(config, id, &done),
    )
}

/// How protection stands: whether it is working, the room it takes, whether old snapshots are cleaned
/// up, and what to do about anything that is not. It exits with failure only for what needs doing now.
pub fn health(ctx: &Context) -> Outcome {
    let facts = gather(ctx.backend, &snapall_system::LinuxSystem)?;
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

/// The restore points: the snapshots that one action took together.
pub fn points(ctx: &Context) -> Outcome {
    let found = restore_points(&all_snapshots(ctx.backend)?);
    ctx.show
        .result(render::points(&found), render::points_json(&found))
}

/// Put every area back to one restore point with a single action: shows what would change in each,
/// asks first (unless told not to), takes a safety snapshot per area (all one restore point, so
/// that undoing this is one action too) and reports each area. The running system is left alone.
pub fn restore_all(ctx: &Context, wanted: &Point, dry_run: bool, yes: bool) -> Outcome {
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
    let done = restore_point(ctx.backend, point, fresh_point(ctx.backend, unix_now()));
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
    let facts = setup::gather();
    let report = if json {
        setup::run(ctx.backend, &facts, &mut setup::Quiet, &options)?
    } else {
        setup::run(ctx.backend, &facts, &mut setup::Terminal, &options)?
    };
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
    match snapall_tui::run(ctx.backend, &snapall_system::LinuxSystem, context)? {
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
