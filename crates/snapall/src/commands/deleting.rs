// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deleting a snapshot.

use super::*;
use crate::output::Outcome;
use crate::{ask, output};
use snapall_core::rollback::after_deleting;
use snapall_core::SnapshotInfo;
use snapall_core::{delete_snapshot, find_snapshot, Act, ConfigName, SnapshotId};

pub fn delete(ctx: &Context, area: &ConfigName, id: SnapshotId, destroy: bool) -> Outcome {
    let found = find_snapshot(ctx.backend, area, id)?;
    let approved = match &found {
        Some(snapshot) => ask_before_deleting(area, snapshot, destroy)?,
        None => ask::nothing_to_lose(Act::DeleteSnapshots, &found)?,
    };
    let change = Change::begin(approved, &deleting_one(area), &deleted_one(area), None)?;
    let deleted = delete_snapshot(ctx.backend, area, id);
    if deleted.is_ok() {
        change.rollback(after_deleting());
    }
    change.end_with(&deleted);
    deleted?;
    ctx.show.result(
        crate::render::deleted(area, id),
        crate::render::deleted_json(area, id),
    )
}

fn ask_before_deleting(
    area: &ConfigName,
    snapshot: &SnapshotInfo,
    destroy: bool,
) -> Result<ask::Approved, Box<dyn std::error::Error>> {
    let it = snapshot_in_words(area, snapshot);
    output::note(&what_deleting_loses(&it, area));
    ask::before(Act::DeleteSnapshots, destroy, &delete_question(&it))
}

fn snapshot_in_words(area: &ConfigName, snapshot: &SnapshotInfo) -> String {
    let when = snapshot
        .date
        .as_deref()
        .map_or(String::new(), |date| format!(" from {date}"));
    let note = if snapshot.description.is_empty() {
        String::new()
    } else {
        format!(" (\"{}\")", snapshot.description)
    };
    format!("snapshot #{} of {area}{when}{note}", snapshot.id)
}

fn what_deleting_loses(it: &str, area: &ConfigName) -> String {
    format!("{it} will be deleted for good. Only this saved moment goes: the files in {area} are not touched.\n")
}

fn delete_question(it: &str) -> String {
    format!("Delete {it}?")
}

fn deleting_one(area: &ConfigName) -> String {
    format!("Deleting 1 snapshot of {area}")
}

fn deleted_one(area: &ConfigName) -> String {
    format!("Deleted 1 snapshot of {area}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::SnapshotKind;

    #[test]
    fn a_snapshot_is_named_with_its_moment_and_its_note_when_it_has_them() {
        let home = ConfigName::parse("home").unwrap();
        let mut snapshot = SnapshotInfo {
            id: SnapshotId::new(3).unwrap(),
            kind: SnapshotKind::Single,
            date: Some("2026-10-25 18:15:00".into()),
            user: "root".into(),
            cleanup: None,
            description: "before the theme".into(),
            point: None,
            used_bytes: None,
        };
        assert_eq!(
            snapshot_in_words(&home, &snapshot),
            "snapshot #3 of home from 2026-10-25 18:15:00 (\"before the theme\")"
        );
        snapshot.description.clear();
        assert_eq!(
            snapshot_in_words(&home, &snapshot),
            "snapshot #3 of home from 2026-10-25 18:15:00"
        );
        snapshot.date = None;
        assert_eq!(snapshot_in_words(&home, &snapshot), "snapshot #3 of home");
    }

    #[test]
    fn what_is_lost_is_said_in_words_that_assume_nothing() {
        let home = ConfigName::parse("home").unwrap();
        let said = what_deleting_loses("snapshot #3 of home", &home);
        assert!(
            said.contains("deleted for good") && said.contains("the files in home are not touched"),
            "{said}"
        );
    }
}
