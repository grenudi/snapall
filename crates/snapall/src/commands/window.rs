// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window: opening it, on the whole or on one file.

use super::*;
use crate::output::Outcome;
use crate::{advice, boot_probe, setup};
use snapall_core::boot;
use snapall_core::{area_covering, SnapshotBackend};
use std::error::Error;
use std::io::IsTerminal;
use std::process::ExitCode;

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

pub(super) fn require_a_terminal() -> Result<(), Box<dyn Error>> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Ok(())
    } else {
        Err("`snapall tui` needs an interactive terminal".into())
    }
}

/// The file the UI opens on, and the config whose snapshots hold its history.
pub(super) fn file_target(
    backend: &dyn SnapshotBackend,
    named: &str,
) -> Result<snapall_tui::FileTarget, Box<dyn Error>> {
    let path = absolute(named)?;
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir()) {
        return Err("directories are not supported yet: choose a file".into());
    }
    let area = area_covering(backend, &path).map_err(|e| advice::explain(&e, &path))?;
    Ok(snapall_tui::FileTarget { path, area })
}

/// What the UI is told about this machine. It is looked at once, here, because the UI's model must
/// not read the machine itself.
pub(super) fn ui_context(
    ctx: &Context,
    file: Option<snapall_tui::FileTarget>,
) -> snapall_tui::UiContext {
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

/// The absolute path of what the user named. A relative path is taken from the working directory,
/// and symlinks in the *directory* part are resolved, so that a path reached through `/home` being
/// a link to `/var/home` is found in the subvolume that really holds it. The file's own name is
/// kept as given: a symlink is itself what is asked about, and a deleted file has nothing to resolve.
pub(super) fn absolute(path: &str) -> Result<String, Box<dyn Error>> {
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
pub(super) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
