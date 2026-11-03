// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Building the request from what was resolved: one command to one request.

use super::*;
use crate::contract::{Command, Contract};
use argenv_resolve::generate::Shell;
use argenv_resolve::{InputResolveExt, Resolution};
use snapall_core::{AreaName, Description, Point, SnapshotId};

/// A required argument that was not given, worded from the contract's own record, so that the message and the usage line
/// cannot say different things.
fn required<T, C>(
    input: &argenv_resolve::Input<T, C>,
    resolution: &Resolution,
) -> Result<T, UsageError>
where
    T: 'static + serde::Serialize,
    C: 'static + serde::Serialize,
    argenv_resolve::Input<T, C>: InputResolveExt<T>,
{
    input.get_from(resolution).ok_or_else(|| {
        usage_error(format!(
            "missing required argument {}",
            input.to_record().usage()
        ))
    })
}

/// [`required`], and what the argument is, for one whose name alone does not say.
fn required_meaning<T, C>(
    input: &argenv_resolve::Input<T, C>,
    resolution: &Resolution,
    meaning: &str,
) -> Result<T, UsageError>
where
    T: 'static + serde::Serialize,
    C: 'static + serde::Serialize,
    argenv_resolve::Input<T, C>: InputResolveExt<T>,
{
    required(input, resolution).map_err(|error| usage_error(format!("{error}: {meaning}")))
}

/// The area and snapshot a command acts on (`delete`, `changes`, `restore`).
pub(super) fn target(resolution: &Resolution) -> Result<(AreaName, SnapshotId), UsageError> {
    let missing = |record: argenv_resolve::Record| {
        usage_error(format!("missing required argument {}", record.usage()))
    };
    let area = Contract::TARGET_AREA
        .get_from(resolution)
        .ok_or_else(|| missing(Contract::TARGET_AREA.to_record()))?;
    let id = Contract::SNAPSHOT_ID
        .get_from(resolution)
        .ok_or_else(|| missing(Contract::SNAPSHOT_ID.to_record()))?;
    Ok((
        parse_area("AREA", &area)?,
        SnapshotId::new(id).ok_or_else(|| {
            usage_error("ID 0 is the current system, not a snapshot; there is nothing to act on")
        })?,
    ))
}

/// A path as text. Paths that are not valid UTF-8 are refused up front, because everything downstream
/// (snapper's output, the audit log, JSON) is text.
pub(super) fn utf8(path: &std::path::Path) -> Result<String, UsageError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| usage_error("a path must be valid UTF-8"))
}

/// The optional PATH argument, as text (see [`utf8`]).
pub(super) fn path_argument(resolution: &Resolution) -> Result<Option<String>, UsageError> {
    Contract::PATH
        .get_from(resolution)
        .map(|path| utf8(&path))
        .transpose()
}

pub(super) fn build_request(
    command: Command,
    resolution: &Resolution,
    dry_run: bool,
) -> Result<Request, UsageError> {
    Ok(match command {
        Command::Create => Request::Create {
            only: Contract::ONLY
                .get_from(resolution)
                .map(|raw| parse_only(&raw))
                .transpose()?,
            description: match Contract::DESCRIPTION.get_from(resolution) {
                Some(text) => Description::parse(&text)
                    .map_err(|e| usage_error(format!("description: {e}")))?,
                None => Description::default(),
            },
            dry_run,
            job: Contract::JOB.get_from(resolution),
            group: Contract::GROUP.get_from(resolution),
        },
        Command::List => Request::List,
        Command::Snapshots => Request::Snapshots {
            area: Contract::SNAPSHOTS_AREA
                .get_from(resolution)
                .map(|name| parse_area("AREA", &name))
                .transpose()?,
        },
        Command::Delete => {
            let (area, id) = target(resolution)?;
            Request::Delete {
                area,
                id,
                destroy: flag(&Contract::DESTROY, resolution),
            }
        }
        Command::Changes => {
            let (area, id) = target(resolution)?;
            Request::Changes {
                area,
                id,
                path: path_argument(resolution)?,
            }
        }
        Command::Restore => {
            let (area, id) = target(resolution)?;
            Request::Restore {
                area,
                id,
                path: path_argument(resolution)?,
                dry_run: flag(&Contract::DRY_RUN, resolution),
                yes: flag(&Contract::YES, resolution),
            }
        }
        Command::Doctor => {
            let install = flag(&Contract::INSTALL, resolution);
            let yes = flag(&Contract::YES, resolution);
            if yes && !install {
                return Err(usage_error("--yes only applies together with --install"));
            }
            Request::Doctor { install, yes }
        }
        Command::Completions => {
            let raw = required(&Contract::SHELL, resolution)?;
            Request::Completions {
                shell: Shell::parse(&raw)
                    .ok_or_else(|| usage_error(format!("unknown shell `{raw}`")))?,
            }
        }
        Command::Man => Request::Man {
            source_date_epoch: Contract::SOURCE_DATE_EPOCH.get_from(resolution),
        },
        Command::History => {
            let path = required(&Contract::HISTORY_FILE, resolution)?;
            Request::History { path: utf8(&path)? }
        }
        Command::Points => Request::Points,
        Command::Health => Request::Health,
        Command::Activity => Request::Activity,
        Command::Undo => Request::Undo {
            change: required_meaning(
                &Contract::UNDO_CHANGE,
                resolution,
                "the name of the change to roll back",
            )?,
            yes: flag(&Contract::YES, resolution),
            destroy: flag(&Contract::DESTROY, resolution),
        },
        Command::Unprotect => Request::Unprotect {
            area: required_meaning(
                &Contract::UNPROTECT_AREA,
                resolution,
                "which area to stop protecting",
            )?,
            dry_run: flag(&Contract::DRY_RUN, resolution),
            destroy: flag(&Contract::DESTROY, resolution),
        },
        Command::Limits => Request::Limits {
            area: required_meaning(&Contract::LIMITS_AREA, resolution, "which area")?,
            keep: Contract::LIMITS_KEEP.get_from(resolution),
            min_age: Contract::LIMITS_MIN_AGE.get_from(resolution),
            cleanup: Contract::LIMITS_CLEANUP.get_from(resolution),
            dry_run: flag(&Contract::DRY_RUN, resolution),
            yes: flag(&Contract::YES, resolution),
            destroy: flag(&Contract::DESTROY, resolution),
        },
        Command::Groups => Request::Groups {
            action: Contract::GROUPS_ACTION.get_from(resolution),
            name: Contract::GROUPS_NAME.get_from(resolution),
            areas: Contract::GROUPS_AREAS.get_from(resolution),
            yes: flag(&Contract::YES, resolution),
        },
        Command::Cancel => Request::Cancel {
            job: required_meaning(
                &Contract::CANCEL_JOB,
                resolution,
                "the name of the change to stop",
            )?,
            yes: flag(&Contract::YES, resolution),
        },
        Command::Quotas => Request::Quotas {
            state: Contract::QUOTAS_STATE.get_from(resolution),
            yes: flag(&Contract::YES, resolution),
        },
        Command::RestoreAll => Request::RestoreAll {
            point: {
                let raw = required(&Contract::RESTORE_POINT, resolution)?;
                Point::parse(&raw).map_err(|e| usage_error(format!("POINT: {e}")))?
            },
            yes: flag(&Contract::YES, resolution),
            dry_run: flag(&Contract::DRY_RUN, resolution),
            job: Contract::JOB.get_from(resolution),
        },
        Command::Setup => Request::Setup {
            area: Contract::SETUP_AREA
                .get_from(resolution)
                .map(|path| utf8(&path))
                .transpose()?,
            yes: flag(&Contract::YES, resolution),
            dry_run: flag(&Contract::DRY_RUN, resolution),
        },
        Command::Tui => Request::Tui {
            file: Contract::TUI_FILE
                .get_from(resolution)
                .map(|path| utf8(&path))
                .transpose()?,
        },
        Command::Contract => Request::Contract,
    })
}

pub(super) fn parse_area(what: &str, name: &str) -> Result<AreaName, UsageError> {
    AreaName::parse(name).map_err(|e| usage_error(format!("{what}: {e}")))
}

/// `--only a,b`: a comma-separated list of area names, each validated.
pub(crate) fn parse_only(raw: &str) -> Result<Vec<AreaName>, UsageError> {
    raw.split(',')
        .map(|name| parse_area("--only", name.trim()))
        .collect()
}
