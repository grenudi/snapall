// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Adapter: the `SnapshotBackend` port, implemented by running the `snapper` CLI.
//!
//! The process boundary is itself a small port (`CommandRunner`) so the exact command
//! lines and the output parsing are unit-tested without a real snapper.

use std::collections::BTreeMap;
use std::io;
use std::process::Command;

use serde::Deserialize;
use snapall_core::{
    BackendError, ConfigName, Description, SnapshotBackend, SnapshotId, SnapshotInfo, SnapshotKind,
};

const PROGRAM: &str = "snapper";

/// What a finished command left behind.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub success: bool,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
}

/// Runs a program with arguments. No shell is ever involved.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[String]) -> io::Result<CommandOutput>;
}

/// Runs real processes.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> io::Result<CommandOutput> {
        let out = Command::new(program).args(args).output()?;
        Ok(CommandOutput {
            success: out.status.success(),
            status: out.status.to_string(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

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

#[derive(Deserialize)]
struct ConfigsDocument {
    configs: Vec<ConfigRow>,
}

#[derive(Deserialize)]
struct ConfigRow {
    config: String,
}

/// One row of `snapper --jsonout -c <config> list`. Fields we do not use are ignored, so a
/// newer snapper that adds columns keeps working.
#[derive(Deserialize)]
struct SnapshotRow {
    number: u64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "pre-number", default)]
    pre_number: Option<u64>,
    #[serde(default)]
    date: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    cleanup: String,
    #[serde(default)]
    description: String,
}

fn non_empty(text: String) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn snapshot_from_row(row: SnapshotRow) -> Result<Option<SnapshotInfo>, BackendError> {
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
    }))
}

fn unexpected(detail: impl Into<String>) -> BackendError {
    BackendError::UnexpectedOutput {
        program: PROGRAM.to_owned(),
        detail: detail.into(),
    }
}

impl<R: CommandRunner> SnapshotBackend for SnapperCli<R> {
    fn list_configs(&self) -> Result<Vec<ConfigName>, BackendError> {
        let output = self.run(self.args(&["--jsonout", "list-configs"]))?;
        let document: ConfigsDocument = serde_json::from_str(&output.stdout)
            .map_err(|e| unexpected(format!("list-configs is not the expected JSON: {e}")))?;
        document
            .configs
            .iter()
            .map(|row| {
                ConfigName::parse(&row.config)
                    .map_err(|e| unexpected(format!("config `{}`: {e}", row.config)))
            })
            .collect()
    }

    fn create_snapshot(
        &self,
        config: &ConfigName,
        description: &Description,
    ) -> Result<SnapshotId, BackendError> {
        let output = self.run(self.args(&[
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
        ]))?;
        SnapshotId::parse(&output.stdout).map_err(|_| {
            unexpected(format!(
                "expected a snapshot number, got `{}`",
                output.stdout.trim()
            ))
        })
    }

    fn list_snapshots(&self, config: &ConfigName) -> Result<Vec<SnapshotInfo>, BackendError> {
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

    fn delete_snapshot(&self, config: &ConfigName, id: SnapshotId) -> Result<(), BackendError> {
        let number = id.to_string();
        self.run(self.args(&["-c", config.as_str(), "delete", number.as_str()]))
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;

    /// Records every call and answers from a script.
    struct Scripted {
        calls: RefCell<Vec<Vec<String>>>,
        replies: RefCell<VecDeque<io::Result<CommandOutput>>>,
    }

    impl Scripted {
        fn new(replies: Vec<io::Result<CommandOutput>>) -> Self {
            Self {
                calls: RefCell::default(),
                replies: RefCell::new(replies.into()),
            }
        }
    }

    impl CommandRunner for &Scripted {
        fn run(&self, program: &str, args: &[String]) -> io::Result<CommandOutput> {
            assert_eq!(program, "snapper");
            self.calls.borrow_mut().push(args.to_vec());
            self.replies
                .borrow_mut()
                .pop_front()
                .expect("scripted reply")
        }
    }

    fn ok(stdout: &str) -> io::Result<CommandOutput> {
        Ok(CommandOutput {
            success: true,
            status: "exit status: 0".into(),
            stdout: stdout.into(),
            stderr: String::new(),
        })
    }

    fn failed(stderr: &str) -> io::Result<CommandOutput> {
        Ok(CommandOutput {
            success: false,
            status: "exit status: 1".into(),
            stdout: String::new(),
            stderr: stderr.into(),
        })
    }

    fn name(s: &str) -> ConfigName {
        ConfigName::parse(s).unwrap()
    }

    #[test]
    fn lists_configs_from_snapper_json() {
        let json = r#"{"configs":[{"config":"root","subvolume":"/"},{"config":"home","subvolume":"/home"}]}"#;
        let script = Scripted::new(vec![ok(json)]);
        let configs = SnapperCli::new(&script).list_configs().unwrap();
        assert_eq!(configs, vec![name("root"), name("home")]);
        assert_eq!(script.calls.borrow()[0], ["--jsonout", "list-configs"]);
    }

    #[test]
    fn no_dbus_is_a_global_option_placed_first() {
        let script = Scripted::new(vec![ok(r#"{"configs":[]}"#)]);
        let configs = SnapperCli::new(&script)
            .with_no_dbus(true)
            .list_configs()
            .unwrap();
        assert!(configs.is_empty());
        assert_eq!(
            script.calls.borrow()[0],
            ["--no-dbus", "--jsonout", "list-configs"]
        );
    }

    #[test]
    fn create_uses_the_exact_verified_command_line() {
        let script = Scripted::new(vec![ok("42\n")]);
        let id = SnapperCli::new(&script)
            .create_snapshot(
                &name("home"),
                &Description::parse("before theming").unwrap(),
            )
            .unwrap();
        assert_eq!(id.get(), 42);
        assert_eq!(
            script.calls.borrow()[0],
            [
                "-c",
                "home",
                "create",
                "--type",
                "single",
                "--cleanup-algorithm",
                "number",
                "--print-number",
                "--description",
                "before theming"
            ]
        );
    }

    #[test]
    fn a_description_that_looks_like_an_option_stays_one_argument() {
        let script = Scripted::new(vec![ok("7")]);
        SnapperCli::new(&script)
            .create_snapshot(&name("root"), &Description::parse("--help").unwrap())
            .unwrap();
        assert_eq!(script.calls.borrow()[0].last().unwrap(), "--help");
    }

    #[test]
    fn a_failing_command_reports_its_stderr() {
        let script = Scripted::new(vec![failed("Config 'nope' not found.\n")]);
        let err = SnapperCli::new(&script)
            .create_snapshot(&name("nope"), &Description::default())
            .unwrap_err();
        match err {
            BackendError::CommandFailed { stderr, .. } => {
                assert_eq!(stderr, "Config 'nope' not found.")
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn a_missing_binary_is_unavailable() {
        let script = Scripted::new(vec![Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no such file",
        ))]);
        let err = SnapperCli::new(&script).list_configs().unwrap_err();
        assert!(matches!(err, BackendError::Unavailable { .. }));
    }

    #[test]
    fn garbage_output_is_unexpected_output_not_a_panic() {
        let script = Scripted::new(vec![ok("not json"), ok("forty-two")]);
        let backend = SnapperCli::new(&script);
        assert!(matches!(
            backend.list_configs(),
            Err(BackendError::UnexpectedOutput { .. })
        ));
        assert!(matches!(
            backend.create_snapshot(&name("root"), &Description::default()),
            Err(BackendError::UnexpectedOutput { .. })
        ));
    }

    #[test]
    fn a_config_name_snapall_cannot_pass_safely_is_rejected_explicitly() {
        let script = Scripted::new(vec![ok(r#"{"configs":[{"config":"--evil"}]}"#)]);
        assert!(matches!(
            SnapperCli::new(&script).list_configs(),
            Err(BackendError::UnexpectedOutput { .. })
        ));
    }

    /// Captured from a real snapper 0.10.6 on a Btrfs volume (config `snapall_it_a`).
    const REAL_LIST: &str = r#"{
      "snapall_it_a": [
        {"subvolume": "/x", "number": 0, "default": false, "active": false, "type": "single",
         "pre-number": null, "date": "", "user": "root", "used-space": null, "cleanup": "",
         "description": "current", "userdata": null},
        {"subvolume": "/x", "number": 3, "default": false, "active": false, "type": "pre",
         "pre-number": null, "date": "2026-10-23 02:36:05", "user": "root", "used-space": null,
         "cleanup": "", "description": "pre/post pair", "userdata": null},
        {"subvolume": "/x", "number": 1, "default": false, "active": false, "type": "single",
         "pre-number": null, "date": "2026-10-23 02:36:05", "user": "root", "used-space": null,
         "cleanup": "number", "description": "first one", "userdata": {"important": "yes"}},
        {"subvolume": "/x", "number": 4, "default": false, "active": false, "type": "post",
         "pre-number": 3, "date": "2026-10-23 02:36:06", "user": "root", "used-space": null,
         "cleanup": "", "description": "pre/post pair", "userdata": null}
      ]
    }"#;

    #[test]
    fn lists_snapshots_oldest_first_without_the_current_system() {
        let script = Scripted::new(vec![ok(REAL_LIST)]);
        let snapshots = SnapperCli::new(&script)
            .list_snapshots(&name("snapall_it_a"))
            .unwrap();
        assert_eq!(
            script.calls.borrow()[0],
            ["--jsonout", "-c", "snapall_it_a", "list"]
        );
        let ids: Vec<u64> = snapshots.iter().map(|s| s.id.get()).collect();
        assert_eq!(ids, [1, 3, 4]);

        let first = &snapshots[0];
        assert_eq!(first.kind, SnapshotKind::Single);
        assert_eq!(first.cleanup.as_deref(), Some("number"));
        assert_eq!(first.date.as_deref(), Some("2026-10-23 02:36:05"));
        assert_eq!(first.description, "first one");

        assert_eq!(snapshots[1].kind, SnapshotKind::Pre);
        assert_eq!(snapshots[1].cleanup, None);
        assert_eq!(
            snapshots[2].kind,
            SnapshotKind::Post {
                pre: SnapshotId::new(3).unwrap()
            }
        );
    }

    #[test]
    fn a_config_with_only_the_current_system_has_no_snapshots() {
        let json = r#"{"home":[{"number":0,"type":"single","pre-number":null,"date":"","user":"root","cleanup":"","description":"current"}]}"#;
        let script = Scripted::new(vec![ok(json)]);
        assert!(SnapperCli::new(&script)
            .list_snapshots(&name("home"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn malformed_snapshot_listings_are_errors_not_guesses() {
        let post_without_pre = r#"{"home":[{"number":2,"type":"post","pre-number":null,"date":"","user":"","cleanup":"","description":""}]}"#;
        let weird_type = r#"{"home":[{"number":2,"type":"hologram","pre-number":null,"date":"","user":"","cleanup":"","description":""}]}"#;
        let other_config = r#"{"root":[]}"#;
        for json in [post_without_pre, weird_type, other_config, "[]", "nope"] {
            let script = Scripted::new(vec![ok(json)]);
            assert!(
                matches!(
                    SnapperCli::new(&script).list_snapshots(&name("home")),
                    Err(BackendError::UnexpectedOutput { .. })
                ),
                "{json}"
            );
        }
    }

    #[test]
    fn delete_passes_the_id_as_its_own_argument() {
        let script = Scripted::new(vec![ok("")]);
        SnapperCli::new(&script)
            .with_no_dbus(true)
            .delete_snapshot(&name("home"), SnapshotId::new(12).unwrap())
            .unwrap();
        assert_eq!(
            script.calls.borrow()[0],
            ["--no-dbus", "-c", "home", "delete", "12"]
        );
    }

    #[test]
    fn a_failed_delete_reports_snappers_message() {
        let script = Scripted::new(vec![failed("Snapshot '999' not found.\n")]);
        let err = SnapperCli::new(&script)
            .delete_snapshot(&name("home"), SnapshotId::new(999).unwrap())
            .unwrap_err();
        assert!(
            matches!(err, BackendError::CommandFailed { ref stderr, .. } if stderr == "Snapshot '999' not found.")
        );
    }
}
