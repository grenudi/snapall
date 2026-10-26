// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stopping the protection of an area: the one thing snapall does that cannot be taken back. Every snapshot
//! of the area is deleted for good (its own files are not touched), so it is *planned* first: what would go,
//! how much room that frees, which restore points and groups it affects. Whoever asks, the plan is shown
//! before anything is done.
//!
//! The system area is refused, both when planning and when doing: its snapshots are what the boot menu
//! offers to start the computer from, and how you get back from a bad update.

use thiserror::Error;

use crate::coverage::describe;
use crate::domain::ConfigName;
use crate::groups::{GroupName, Groups};
use crate::health::format_bytes;
use crate::points::restore_points;
use crate::port::{BackendError, SnapshotBackend};
use crate::usecase::{all_snapshots, is_root, SnapshotError};

/// What stopping the protection of one area would do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub config: ConfigName,
    /// Where the area is.
    pub subvolume: String,
    /// How many snapshots would be deleted.
    pub snapshots: usize,
    /// The room that would be freed, when every snapshot's own room is known (see quotas).
    pub freed_bytes: Option<u64>,
    /// How many restore points include this area, and so would stop putting it back.
    pub points: usize,
    /// The groups that name it.
    pub groups: Vec<GroupName>,
}

#[derive(Debug, Error)]
pub enum UnprotectError {
    #[error("snapall will not stop protecting the system area. Its snapshots are what the boot menu offers to start the computer from, and how you get back from a bad update: removing them would take that away.")]
    SystemArea,
    #[error("`{0}` is not a protected area")]
    Unknown(String),
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error(transparent)]
    Snapshots(#[from] SnapshotError),
    #[error("snapper still lists `{0}` as protected after it was asked to stop")]
    StillProtected(String),
}

/// What it would do, or why it would not.
pub fn plan(
    backend: &dyn SnapshotBackend,
    groups: &Groups,
    config: &ConfigName,
) -> Result<Plan, UnprotectError> {
    if !backend.list_configs()?.contains(config) {
        return Err(UnprotectError::Unknown(config.to_string()));
    }
    let subvolume = backend.config_subvolume(config)?;
    if is_root(&subvolume) {
        return Err(UnprotectError::SystemArea);
    }
    let snapshots = backend.list_snapshots(config)?;
    let measured: Option<u64> = snapshots.iter().map(|s| s.used_bytes).sum();
    let points = restore_points(&all_snapshots(backend)?)
        .iter()
        .filter(|point| point.members.iter().any(|member| member.config == *config))
        .count();
    let naming = groups
        .names()
        .filter(|name| groups.get(name).is_some_and(|areas| areas.contains(config)))
        .cloned()
        .collect();
    Ok(Plan {
        config: config.clone(),
        subvolume,
        snapshots: snapshots.len(),
        freed_bytes: if snapshots.is_empty() { None } else { measured },
        points,
        groups: naming,
    })
}

/// Do it: the area stops being protected and its snapshots are deleted. Returns how many were. The system
/// area is refused here too, so that no caller can reach the deletion without passing the check.
pub fn stop_protecting(
    backend: &dyn SnapshotBackend,
    plan: &Plan,
) -> Result<usize, UnprotectError> {
    if is_root(&backend.config_subvolume(&plan.config)?) {
        return Err(UnprotectError::SystemArea);
    }
    backend.delete_config(&plan.config)?;
    if backend.list_configs()?.contains(&plan.config) {
        return Err(UnprotectError::StillProtected(plan.config.to_string()));
    }
    Ok(plan.snapshots)
}

impl Plan {
    /// What it will do, in plain sentences: the same words for the window and the command line.
    pub fn words(&self) -> Vec<String> {
        let title = describe(&self.subvolume).title;
        let snapshots = match (self.snapshots, self.freed_bytes) {
            (0, _) => "It has no snapshots to delete.".to_owned(),
            (1, Some(bytes)) => format!(
                "Its 1 snapshot will be deleted for good, freeing {}.",
                format_bytes(bytes)
            ),
            (n, Some(bytes)) => format!(
                "Its {n} snapshots will be deleted for good, freeing {}.",
                format_bytes(bytes)
            ),
            (1, None) => "Its 1 snapshot will be deleted for good.".to_owned(),
            (n, None) => format!("Its {n} snapshots will be deleted for good."),
        };
        let mut lines = vec![format!(
            "{title} ({}) will no longer be protected. {snapshots} The files in it are not touched.",
            self.subvolume
        )];
        if self.points > 0 {
            lines.push(format!(
                "{} restore point{} include{} it and will no longer put it back.",
                self.points,
                if self.points == 1 { "" } else { "s" },
                if self.points == 1 { "s" } else { "" }
            ));
        }
        if !self.groups.is_empty() {
            let names: Vec<String> = self.groups.iter().map(ToString::to_string).collect();
            lines.push(format!(
                "The group{} {} name{} it, and will say it is no longer protected until you change {}.",
                if names.len() == 1 { "" } else { "s" },
                names.join(", "),
                if names.len() == 1 { "s" } else { "" },
                if names.len() == 1 { "it" } else { "them" }
            ));
        }
        lines.push(
            "This cannot be undone. Protecting it again later starts fresh, with no snapshots."
                .to_owned(),
        );
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Description;
    use crate::testing::InMemoryBackend;
    use crate::usecase::{plan_snapshots, run_plan};

    fn area(name: &str) -> ConfigName {
        ConfigName::parse(name).unwrap()
    }

    fn machine() -> InMemoryBackend {
        let backend =
            InMemoryBackend::with_configs(&["data", "home", "root"]).with_root_config("root");
        let everything = plan_snapshots(&backend, None).unwrap();
        let point = crate::domain::Point::parse("20261025T181500Z").unwrap();
        run_plan(
            &backend,
            &everything,
            &Description::parse("one").unwrap(),
            Some(&point),
        );
        run_plan(
            &backend,
            &everything,
            &Description::parse("two").unwrap(),
            None,
        );
        backend
    }

    #[test]
    fn the_plan_counts_what_would_go_and_what_it_would_touch() {
        let backend = machine();
        let mut groups = Groups::default();
        groups
            .set(
                GroupName::parse("theming").unwrap(),
                vec![area("home"), area("data")],
            )
            .unwrap();
        let plan = plan(&backend, &groups, &area("home")).unwrap();
        assert_eq!(
            (plan.snapshots, plan.points),
            (2, 2),
            "two snapshots, each from an action that made a restore point"
        );
        assert_eq!(plan.groups, [GroupName::parse("theming").unwrap()]);
        assert_eq!(
            plan.freed_bytes, None,
            "the room is not known without quotas"
        );
    }

    #[test]
    fn the_system_area_is_refused_when_planning_and_when_doing() {
        let backend = machine();
        assert!(matches!(
            plan(&backend, &Groups::default(), &area("root")),
            Err(UnprotectError::SystemArea)
        ));
        // a plan made some other way cannot get past the second check
        let forged = Plan {
            config: area("root"),
            subvolume: "/".into(),
            snapshots: 2,
            freed_bytes: None,
            points: 0,
            groups: vec![],
        };
        assert!(matches!(
            stop_protecting(&backend, &forged),
            Err(UnprotectError::SystemArea)
        ));
        assert!(
            backend.list_configs().unwrap().contains(&area("root")),
            "nothing was removed"
        );
        assert!(UnprotectError::SystemArea
            .to_string()
            .contains("how you get back from a bad update"));
    }

    #[test]
    fn an_unknown_area_is_refused() {
        let backend = machine();
        assert!(matches!(
            plan(&backend, &Groups::default(), &area("nope")),
            Err(UnprotectError::Unknown(_))
        ));
    }

    #[test]
    fn stopping_removes_that_area_and_its_snapshots_and_nothing_else() {
        let backend = machine();
        let plan = plan(&backend, &Groups::default(), &area("home")).unwrap();
        assert_eq!(stop_protecting(&backend, &plan).unwrap(), 2);
        let left = backend.list_configs().unwrap();
        assert_eq!(left, [area("data"), area("root")]);
        assert_eq!(
            backend.list_snapshots(&area("data")).unwrap().len(),
            2,
            "the others keep theirs"
        );
        // and the restore point it was in still stands for the others
        let points = restore_points(&all_snapshots(&backend).unwrap());
        assert_eq!(
            points.len(),
            2,
            "both actions are still restore points, of the areas that remain"
        );
        assert!(points
            .iter()
            .all(|p| p.members.iter().all(|m| m.config != area("home"))));
    }

    #[test]
    fn the_words_say_what_goes_what_stays_what_it_touches_and_that_it_cannot_be_undone() {
        let backend = machine();
        let mut groups = Groups::default();
        groups
            .set(GroupName::parse("theming").unwrap(), vec![area("home")])
            .unwrap();
        let said = plan(&backend, &groups, &area("home"))
            .unwrap()
            .words()
            .join("\n");
        for must in [
            "will no longer be protected. Its 2 snapshots will be deleted for good. The files in it are not touched.",
            "2 restore points include it and will no longer put it back.",
            "The group theming names it, and will say it is no longer protected until you change it.",
            "This cannot be undone. Protecting it again later starts fresh, with no snapshots.",
        ] {
            assert!(said.contains(must), "`{must}` missing:\n{said}");
        }
        let bytes = Plan {
            config: area("home"),
            subvolume: "/mnt/home".into(),
            snapshots: 1,
            freed_bytes: Some(3_145_728),
            points: 0,
            groups: vec![],
        };
        assert!(
            bytes.words()[0].contains("Its 1 snapshot will be deleted for good, freeing 3.0 MiB."),
            "{:?}",
            bytes.words()
        );
        let none = Plan {
            snapshots: 0,
            ..bytes
        };
        assert!(none.words()[0].contains("It has no snapshots to delete."));
    }
}
