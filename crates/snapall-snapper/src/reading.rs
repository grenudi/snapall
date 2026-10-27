// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading what snapper prints back into our own words.

use crate::running::PROGRAM;
use serde::Deserialize;
use snapall_core::{
    BackendError, Change, ChangeKind, Point, RestoreSummary, SnapshotId, SnapshotInfo, SnapshotKind,
};

#[derive(Deserialize)]
pub(crate) struct ConfigsDocument {
    pub(crate) configs: Vec<ConfigRow>,
}

#[derive(Deserialize)]
pub(crate) struct ConfigRow {
    pub(crate) config: String,
    #[serde(default)]
    pub(crate) subvolume: String,
}

/// One row of `snapper --jsonout -c <config> list`. Fields we do not use are ignored, so a
/// newer snapper that adds columns keeps working.
#[derive(Deserialize)]
pub(crate) struct SnapshotRow {
    pub(crate) number: u64,
    #[serde(rename = "type")]
    pub(crate) kind: String,
    #[serde(rename = "pre-number", default)]
    pub(crate) pre_number: Option<u64>,
    #[serde(default)]
    pub(crate) date: String,
    #[serde(default)]
    pub(crate) user: String,
    #[serde(default)]
    pub(crate) cleanup: String,
    #[serde(default)]
    pub(crate) description: String,
    /// What was stored with the snapshot (`null` when nothing was).
    #[serde(default)]
    pub(crate) userdata: Option<std::collections::BTreeMap<String, String>>,
    /// The bytes only this snapshot holds: reported when Btrfs quotas are on, `null` otherwise.
    #[serde(rename = "used-space", default)]
    pub(crate) used_space: Option<u64>,
}

/// The key a snapshot's tag is stored under: snapper keeps free `key=value` pairs with a snapshot.
pub(crate) const POINT_KEY: &str = "snapall-point";

pub(crate) fn non_empty(text: String) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub(crate) fn snapshot_from_row(row: SnapshotRow) -> Result<Option<SnapshotInfo>, BackendError> {
    // Number 0 is "the current system", not a snapshot.
    let Some(id) = SnapshotId::new(row.number) else {
        return Ok(None);
    };
    let kind = match (row.kind.as_str(), row.pre_number) {
        ("single", _) => SnapshotKind::Single,
        ("pre", _) => SnapshotKind::Pre,
        ("post", Some(pre)) => SnapshotKind::Post {
            pre: SnapshotId::new(pre).ok_or_else(|| {
                unexpected(format!("snapshot {id}: post-snapshot points at pre 0"))
            })?,
        },
        ("post", None) => {
            return Err(unexpected(format!(
                "snapshot {id} is a post snapshot without a pre-number"
            )))
        }
        (other, _) => {
            return Err(unexpected(format!(
                "snapshot {id} has unknown type `{other}`"
            )))
        }
    };
    Ok(Some(SnapshotInfo {
        id,
        kind,
        date: non_empty(row.date),
        user: row.user,
        cleanup: non_empty(row.cleanup),
        description: row.description,
        point: row
            .userdata
            .as_ref()
            .and_then(|data| data.get(POINT_KEY))
            .and_then(|value| Point::parse(value).ok()),
        used_bytes: row.used_space,
    }))
}

pub(crate) fn unexpected(detail: impl Into<String>) -> BackendError {
    BackendError::UnexpectedOutput {
        program: PROGRAM.to_owned(),
        detail: detail.into(),
    }
}

/// `snapper status A..B` prints one line per path: six status characters, a space, the path.
/// The first character is what matters here: `+` created since, `-` deleted since, anything
/// else a difference in content, type, permissions, ownership or attributes.
pub(crate) fn parse_status(stdout: &str) -> Result<Vec<Change>, BackendError> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut chars = line.chars();
            let kind = chars.next();
            let flags_and_space: String = chars.by_ref().take(6).collect();
            let path: String = chars.collect();
            let well_formed = flags_and_space.chars().count() == 6
                && flags_and_space.ends_with(' ')
                && path.starts_with('/');
            match (kind, well_formed) {
                (Some(kind), true) => Ok(Change {
                    kind: match kind {
                        '+' => ChangeKind::Added,
                        '-' => ChangeKind::Removed,
                        _ => ChangeKind::Changed,
                    },
                    path,
                }),
                _ => Err(unexpected(format!(
                    "a status line that is not `<flags> <path>`: `{line}`"
                ))),
            }
        })
        .collect()
}

/// `snapper undochange` ends with `create:N modify:N delete:N`. If it printed nothing of the
/// kind the restore still happened (it exited successfully), so the counts are zero rather
/// than an error that would suggest it had not.
pub(crate) fn parse_summary(stdout: &str) -> RestoreSummary {
    let count = |key: &str| {
        stdout
            .split_whitespace()
            .find_map(|word| word.strip_prefix(key))
            .and_then(|number| number.parse().ok())
            .unwrap_or(0)
    };
    RestoreSummary {
        created: count("create:"),
        modified: count("modify:"),
        deleted: count("delete:"),
    }
}
