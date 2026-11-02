// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Whether what a rollback would go back to is still there. It is checked when the rollback is asked for, not when it
//! was recorded: snapper's cleanup may have removed the snapshot since, and then the honest answer is that it cannot
//! be done.

use crate::going_back::points::restore_points;
use crate::looking::journal::Undo;
use crate::looking::snapshots::all_snapshots;
use crate::ports::snapshots::SnapshotBackend;
use crate::vocabulary::domain::AreaName;

/// What a rollback would go back to: something that cleanup may have removed since.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoesBackTo {
    Snapshot { area: AreaName, id: u64 },
    Point(String),
}

impl Undo {
    /// The snapshots and restore points its commands name. A command that names neither (deleting what was made,
    /// switching a setting back) goes back to nothing that can disappear.
    pub fn goes_back_to(&self) -> Result<Vec<GoesBackTo>, String> {
        let mut found = Vec::new();
        for command in &self.commands {
            match command
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .as_slice()
            {
                ["restore", area, id, ..] => found.push(GoesBackTo::Snapshot {
                    area: AreaName::parse(area).map_err(|error| error.to_string())?,
                    id: id
                        .parse()
                        .map_err(|_| format!("`{id}` is not a snapshot number"))?,
                }),
                ["restore-all", point, ..] => found.push(GoesBackTo::Point((*point).to_owned())),
                _ => {}
            }
        }
        Ok(found)
    }
}

/// Everything the rollback would go back to is still there, or why it cannot be done.
pub fn still_there(backend: &dyn SnapshotBackend, undo: &Undo) -> Result<(), String> {
    for target in undo.goes_back_to()? {
        match target {
            GoesBackTo::Snapshot { area, id } => {
                let there = backend
                    .list_snapshots(&area)
                    .map_err(|error| error.to_string())?
                    .iter()
                    .any(|snapshot| snapshot.id.get() == id);
                if !there {
                    return Err(format!(
                        "snapshot #{id} of {area}, which this would go back to, no longer exists: cleanup has removed it, so this cannot be rolled back"
                    ));
                }
            }
            GoesBackTo::Point(point) => {
                let all = all_snapshots(backend).map_err(|error| error.to_string())?;
                if !restore_points(&all)
                    .iter()
                    .any(|candidate| candidate.point.to_string() == point)
                {
                    return Err(format!(
                        "restore point {point}, which this would go back to, no longer exists: cleanup has removed it, so this cannot be rolled back"
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::going_back::rollback::{after_restoring, after_restoring_all};
    use crate::looking::journal::Rollback;
    use crate::ports::testing::InMemoryBackend;
    use crate::vocabulary::consequence::Weight;
    use crate::vocabulary::domain::{Description, Point};

    fn undo(commands: &[&[&str]]) -> Undo {
        Undo {
            what: "put it back".into(),
            commands: commands
                .iter()
                .map(|words| words.iter().map(|w| (*w).to_owned()).collect())
                .collect(),
            weight: Weight::Changes,
        }
    }

    fn two_areas_with_a_point() -> (InMemoryBackend, Point) {
        let backend = InMemoryBackend::with_areas(&["docs", "home"]);
        let point = Point::parse("20261025T100000Z").unwrap();
        let words = Description::parse("before theming").unwrap();
        for area in ["docs", "home"] {
            backend
                .create_snapshot_in(&AreaName::parse(area).unwrap(), &words, &point)
                .unwrap();
        }
        (backend, point)
    }

    #[test]
    fn a_snapshot_that_is_still_there_can_be_gone_back_to_and_one_that_cleanup_removed_cannot() {
        let (backend, _) = two_areas_with_a_point();
        let (area, id, _) = backend.snapshots().into_iter().next().unwrap();
        let command = ["restore", area.as_str(), &id.get().to_string()];
        assert_eq!(still_there(&backend, &undo(&[&command])), Ok(()));
        backend.delete_snapshot(&area, id).unwrap();
        let gone = still_there(&backend, &undo(&[&command])).unwrap_err();
        assert!(
            gone.contains("no longer exists: cleanup has removed it"),
            "{gone}"
        );
        assert!(gone.contains(&format!("snapshot #{}", id.get())), "{gone}");
    }

    #[test]
    fn a_restore_point_is_there_until_one_of_its_snapshots_is_removed() {
        let (backend, point) = two_areas_with_a_point();
        let command = ["restore-all", "20261025T100000Z"];
        assert_eq!(point.to_string(), "20261025T100000Z");
        assert_eq!(still_there(&backend, &undo(&[&command])), Ok(()));
        let (area, id, _) = backend.snapshots().into_iter().next().unwrap();
        backend.delete_snapshot(&area, id).unwrap();
        let gone = still_there(&backend, &undo(&[&command])).unwrap_err();
        assert!(gone.starts_with("restore point 20261025T100000Z"), "{gone}");
    }

    #[test]
    fn words_that_name_nothing_that_can_disappear_go_back_to_nothing() {
        let (backend, _) = two_areas_with_a_point();
        let delete = undo(&[&["delete", "home", "3"], &["groups", "remove", "theming"]]);
        assert_eq!(delete.goes_back_to(), Ok(vec![]));
        assert_eq!(still_there(&backend, &delete), Ok(()));
    }

    #[test]
    fn a_snapshot_number_that_is_not_a_number_is_said_so() {
        let bad = undo(&[&["restore", "home", "twelve"]]);
        assert_eq!(
            bad.goes_back_to(),
            Err("`twelve` is not a snapshot number".to_owned())
        );
    }

    /// What each producer writes and what this reads are one language: if a producer changes how it words a command, this
    /// fails here, and the rollback is not found to be broken only when someone needs it.
    #[test]
    fn every_way_back_the_core_writes_is_one_this_can_read() {
        let area = AreaName::parse("home").unwrap();
        let safety = crate::vocabulary::domain::SnapshotId::new(12).unwrap();
        let produced = [
            after_restoring(&area, None, safety),
            after_restoring(&area, Some("/etc/x"), safety),
            after_restoring_all("20261025T100000Z"),
        ];
        let named: Vec<Vec<GoesBackTo>> = produced
            .iter()
            .map(|rollback| match rollback {
                Rollback::Undo(undo) => undo.goes_back_to().unwrap(),
                Rollback::Advice(_) => vec![],
            })
            .collect();
        assert_eq!(
            named[0],
            [GoesBackTo::Snapshot {
                area: area.clone(),
                id: 12
            }]
        );
        assert_eq!(named[1], [GoesBackTo::Snapshot { area, id: 12 }]);
        assert_eq!(named[2], [GoesBackTo::Point("20261025T100000Z".to_owned())]);
    }
}
