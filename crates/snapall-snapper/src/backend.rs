// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Talking to snapper: our snapshot verbs, said to it one command at a time.

use crate::files::*;
use crate::reading::*;
use crate::running::*;
use snapall_core::{
    AreaName, AreaOption, BackendError, Change, Description, FileVersion, Point, RestoreSummary,
    Retention, SnapshotBackend, SnapshotId, SnapshotInfo,
};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SnapperCli<R = SystemRunner> {
    runner: R,
    no_dbus: bool,
}

impl SnapperCli<SystemRunner> {
    pub fn system() -> Self {
        Self::new(SystemRunner)
    }
}

impl<R: CommandRunner> SnapperCli<R> {
    /// One `snapper create`, tagged when it belongs to a restore point.
    fn create(
        &self,
        config: &AreaName,
        description: &Description,
        point: Option<&Point>,
    ) -> Result<SnapshotId, BackendError> {
        let tag = point.map(|p| format!("{POINT_KEY}={p}"));
        let mut args = vec![
            "-c",
            config.as_str(),
            "create",
            "--type",
            "single",
            "--cleanup-algorithm",
            "number",
            "--print-number",
            "--description",
            description.as_str(),
        ];
        if let Some(tag) = &tag {
            args.extend(["--userdata", tag.as_str()]);
        }
        let output = self.run(self.args(&args))?;
        SnapshotId::parse(&output.stdout).map_err(|_| {
            unexpected(format!(
                "expected a snapshot number, got `{}`",
                output.stdout.trim()
            ))
        })
    }

    pub fn new(runner: R) -> Self {
        Self {
            runner,
            no_dbus: false,
        }
    }

    /// Talk to libsnapper directly instead of through snapperd (root only).
    /// Meant for containers and CI where there is no system D-Bus.
    pub fn with_no_dbus(mut self, no_dbus: bool) -> Self {
        self.no_dbus = no_dbus;
        self
    }

    fn args(&self, rest: &[&str]) -> Vec<String> {
        let mut args = Vec::with_capacity(rest.len() + 1);
        if self.no_dbus {
            args.push("--no-dbus".to_owned());
        }
        args.extend(rest.iter().map(|s| (*s).to_owned()));
        args
    }

    fn run(&self, args: Vec<String>) -> Result<CommandOutput, BackendError> {
        let output = self
            .runner
            .run(PROGRAM, &args)
            .map_err(|e| BackendError::Unavailable {
                program: PROGRAM.to_owned(),
                detail: e.to_string(),
            })?;
        if output.success {
            Ok(output)
        } else {
            let stderr = output.stderr.trim();
            let detail = if stderr.is_empty() {
                output.stdout.trim()
            } else {
                stderr
            };
            Err(BackendError::CommandFailed {
                program: PROGRAM.to_owned(),
                status: output.status,
                stderr: detail.to_owned(),
            })
        }
    }
}

impl<R: CommandRunner> SnapshotBackend for SnapperCli<R> {
    fn list_areas(&self) -> Result<Vec<AreaName>, BackendError> {
        let output = self.run(self.args(&["--jsonout", "list-configs"]))?;
        let document: ConfigsDocument = serde_json::from_str(&output.stdout)
            .map_err(|e| unexpected(format!("list-configs is not the expected JSON: {e}")))?;
        document
            .configs
            .iter()
            .map(|row| {
                AreaName::parse(&row.config)
                    .map_err(|e| unexpected(format!("config `{}`: {e}", row.config)))
            })
            .collect()
    }

    fn create_snapshot(
        &self,
        config: &AreaName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError> {
        self.create(config, description, None)
    }

    fn create_snapshot_in(
        &self,
        config: &AreaName,
        description: &Description,
        point: &Point,
    ) -> Result<SnapshotId, BackendError> {
        self.create(config, description, Some(point))
    }

    fn list_snapshots(&self, config: &AreaName) -> Result<Vec<SnapshotInfo>, BackendError> {
        let output = self.run(self.args(&["--jsonout", "-c", config.as_str(), "list"]))?;
        let mut document: BTreeMap<String, Vec<SnapshotRow>> = serde_json::from_str(&output.stdout)
            .map_err(|e| unexpected(format!("list is not the expected JSON: {e}")))?;
        let rows = document
            .remove(config.as_str())
            .ok_or_else(|| unexpected(format!("list has no entry for config `{config}`")))?;
        let mut snapshots = rows
            .into_iter()
            .filter_map(|row| snapshot_from_row(row).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        snapshots.sort_by_key(|s| s.id);
        Ok(snapshots)
    }

    fn delete_snapshot(&self, config: &AreaName, id: SnapshotId) -> Result<(), BackendError> {
        let number = id.to_string();
        self.run(self.args(&["-c", config.as_str(), "delete", number.as_str()]))
            .map(|_| ())
    }

    fn area_location(&self, config: &AreaName) -> Result<String, BackendError> {
        let output = self.run(self.args(&["--jsonout", "list-configs"]))?;
        let document: ConfigsDocument = serde_json::from_str(&output.stdout)
            .map_err(|e| unexpected(format!("list-configs is not the expected JSON: {e}")))?;
        let row = document
            .configs
            .into_iter()
            .find(|row| row.config == config.as_str())
            .ok_or_else(|| BackendError::UnknownArea(config.clone()))?;
        if row.subvolume.starts_with('/') {
            Ok(row.subvolume)
        } else {
            Err(unexpected(format!(
                "config `{config}` has no absolute subvolume path (`{}`)",
                row.subvolume
            )))
        }
    }

    fn changes_since(
        &self,
        config: &AreaName,
        id: SnapshotId,
    ) -> Result<Vec<Change>, BackendError> {
        let range = format!("{id}..0");
        let output = self.run(self.args(&["-c", config.as_str(), "status", range.as_str()]))?;
        parse_status(&output.stdout)
    }

    fn create_area(&self, name: &AreaName, subvolume: &str) -> Result<(), BackendError> {
        // `subvolume` is absolute (the use case checks), so it can never be read as an option.
        self.run(self.args(&["-c", name.as_str(), "create-config", subvolume]))
            .map(|_| ())
    }

    fn delete_area(&self, name: &AreaName) -> Result<(), BackendError> {
        self.run(self.args(&["-c", name.as_str(), "delete-config"]))
            .map(|_| ())
    }

    fn set_area_option(&self, name: &AreaName, option: AreaOption) -> Result<(), BackendError> {
        let setting = match option {
            AreaOption::TimelineCreate(on) => {
                format!("TIMELINE_CREATE={}", if on { "yes" } else { "no" })
            }
            AreaOption::KeepNewest(count) => format!("NUMBER_LIMIT={count}"),
            AreaOption::NeverRemoveYoungerThan(seconds) => format!("NUMBER_MIN_AGE={seconds}"),
            AreaOption::CleanupOfSnapshots(on) => {
                format!("NUMBER_CLEANUP={}", if on { "yes" } else { "no" })
            }
        };
        self.run(self.args(&["-c", name.as_str(), "set-config", &setting]))
            .map(|_| ())
    }

    fn retention(&self, config: &AreaName) -> Result<Retention, BackendError> {
        let output = self.run(self.args(&["--jsonout", "-c", config.as_str(), "get-config"]))?;
        let settings: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&output.stdout)
                .map_err(|e| unexpected(format!("get-config: {e}")))?;
        Ok(Retention::from_settings(&settings))
    }

    fn covers(&self, config: &AreaName, path: &str) -> Result<bool, BackendError> {
        let subvolume = self.area_location(config)?;
        same_subvolume(Path::new(&subvolume), Path::new(path))
    }

    fn file_versions(
        &self,
        config: &AreaName,
        path: &str,
    ) -> Result<Vec<FileVersion>, BackendError> {
        let subvolume = self.area_location(config)?;
        let base = subvolume.trim_end_matches('/');
        let relative = path
            .strip_prefix(base)
            .map(|rest| rest.trim_start_matches('/'))
            .ok_or_else(|| unexpected(format!("`{path}` is not under `{subvolume}`")))?;
        self.list_snapshots(config)?
            .into_iter()
            .map(|snapshot| {
                // snapper keeps snapshot N of a subvolume at <subvolume>/.snapshots/N/snapshot
                let root = format!("{base}/.snapshots/{}/snapshot", snapshot.id);
                if !Path::new(&root).is_dir() {
                    return Err(BackendError::Unavailable {
                        program: "snapshot directory".into(),
                        detail: format!(
                            "snapshot {} is not at `{root}` (a snapper layout other than the default?)",
                            snapshot.id
                        ),
                    });
                }
                let then = Path::new(&root).join(relative);
                let state = file_state(&then, Path::new(path))?;
                Ok(FileVersion { snapshot, state })
            })
            .collect()
    }

    fn undo_changes(
        &self,
        config: &AreaName,
        id: SnapshotId,
        path: Option<&str>,
    ) -> Result<RestoreSummary, BackendError> {
        let range = format!("{id}..0");
        let mut rest = vec!["-c", config.as_str(), "undochange", range.as_str()];
        if let Some(path) = path {
            // The use case only passes absolute paths, so this can never be read as an option.
            rest.push(path);
        }
        let output = self.run(self.args(&rest))?;
        Ok(parse_summary(&output.stdout))
    }
}
