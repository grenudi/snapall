// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The snapper adapter, driven through its public face with a scripted runner.

#![cfg(test)]

use snapall_core::{
    AreaName, AreaOption, BackendError, ChangeKind, Description, FileState, RestoreSummary,
    Retention, SnapshotBackend, SnapshotId, SnapshotKind,
};
use std::fs::{self};
use std::io;
use std::os::unix::fs::PermissionsExt;

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

fn name(s: &str) -> AreaName {
    AreaName::parse(s).unwrap()
}

#[test]
fn lists_configs_from_snapper_json() {
    let json =
        r#"{"configs":[{"config":"root","subvolume":"/"},{"config":"home","subvolume":"/home"}]}"#;
    let script = Scripted::new(vec![ok(json)]);
    let configs = SnapperCli::new(&script).list_areas().unwrap();
    assert_eq!(configs, vec![name("root"), name("home")]);
    assert_eq!(script.calls.borrow()[0], ["--jsonout", "list-configs"]);
}

#[test]
fn no_dbus_is_a_global_option_placed_first() {
    let script = Scripted::new(vec![ok(r#"{"configs":[]}"#)]);
    let configs = SnapperCli::new(&script)
        .with_no_dbus(true)
        .list_areas()
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
    let err = SnapperCli::new(&script).list_areas().unwrap_err();
    assert!(matches!(err, BackendError::Unavailable { .. }));
}

#[test]
fn garbage_output_is_unexpected_output_not_a_panic() {
    let script = Scripted::new(vec![ok("not json"), ok("forty-two")]);
    let backend = SnapperCli::new(&script);
    assert!(matches!(
        backend.list_areas(),
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
        SnapperCli::new(&script).list_areas(),
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

// ---- restoring ------------------------------------------------------------------------

fn id(n: u64) -> SnapshotId {
    SnapshotId::new(n).unwrap()
}

#[test]
fn the_mount_point_comes_from_list_configs() {
    let json =
        r#"{"configs":[{"config":"home","subvolume":"/home"},{"config":"root","subvolume":"/"}]}"#;
    let script = Scripted::new(vec![ok(json), ok(json), ok(json)]);
    let backend = SnapperCli::new(&script);
    assert_eq!(backend.area_location(&name("root")).unwrap(), "/");
    assert_eq!(backend.area_location(&name("home")).unwrap(), "/home");
    assert!(matches!(
        backend.area_location(&name("nope")),
        Err(BackendError::UnknownArea(_))
    ));
    assert_eq!(script.calls.borrow()[0], ["--jsonout", "list-configs"]);
}

#[test]
fn changes_are_read_from_status_with_the_verified_command_line_and_format() {
    let status = "+..... /home/u/added.txt\n-..... /home/u/gone.txt\nc..... /home/u/edited.txt\n.p.... /home/u/a file with spaces\n";
    let script = Scripted::new(vec![ok(status)]);
    let changes = SnapperCli::new(&script)
        .changes_since(&name("home"), id(14))
        .unwrap();
    assert_eq!(script.calls.borrow()[0], ["-c", "home", "status", "14..0"]);
    let seen: Vec<_> = changes.iter().map(|c| (c.kind, c.path.as_str())).collect();
    assert_eq!(
        seen,
        [
            (ChangeKind::Added, "/home/u/added.txt"),
            (ChangeKind::Removed, "/home/u/gone.txt"),
            (ChangeKind::Changed, "/home/u/edited.txt"),
            (ChangeKind::Changed, "/home/u/a file with spaces"),
        ]
    );
}

#[test]
fn nothing_changed_is_an_empty_list_and_a_malformed_line_is_an_error_not_a_guess() {
    let script = Scripted::new(vec![
        ok(""),
        ok("this is not a status line\n"),
        ok("c..... relative/path\n"),
    ]);
    let backend = SnapperCli::new(&script);
    assert!(backend
        .changes_since(&name("home"), id(1))
        .unwrap()
        .is_empty());
    for _ in 0..2 {
        assert!(matches!(
            backend.changes_since(&name("home"), id(1)),
            Err(BackendError::UnexpectedOutput { .. })
        ));
    }
}

#[test]
fn undoing_uses_undochange_with_the_path_as_its_own_argument() {
    let script = Scripted::new(vec![
        ok("create:1 modify:2 delete:3\n"),
        ok("create:0 modify:1 delete:0\n"),
    ]);
    let backend = SnapperCli::new(&script);
    let all = backend.undo_changes(&name("home"), id(7), None).unwrap();
    assert_eq!((all.created, all.modified, all.deleted), (1, 2, 3));
    let one = backend
        .undo_changes(&name("home"), id(7), Some("/home/u/a b.txt"))
        .unwrap();
    assert_eq!(one.total(), 1);
    let calls = script.calls.borrow();
    assert_eq!(calls[0], ["-c", "home", "undochange", "7..0"]);
    assert_eq!(
        calls[1],
        ["-c", "home", "undochange", "7..0", "/home/u/a b.txt"]
    );
}

#[test]
fn a_successful_undo_that_prints_no_summary_is_still_a_success() {
    let script = Scripted::new(vec![ok("")]);
    let done = SnapperCli::new(&script)
        .undo_changes(&name("home"), id(1), None)
        .unwrap();
    assert_eq!(done, RestoreSummary::default());
}

#[test]
fn a_failed_undo_reports_snappers_message() {
    let script = Scripted::new(vec![failed("Snapshot '99' not found.")]);
    let error = SnapperCli::new(&script)
        .undo_changes(&name("home"), id(99), None)
        .unwrap_err();
    assert!(
        error.to_string().contains("Snapshot '99' not found"),
        "{error}"
    );
}

// ---- file history ---------------------------------------------------------------------

/// A fake subvolume on disk, laid out the way snapper lays out its snapshots.
struct Volume(std::path::PathBuf);

impl Volume {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!("snapall-fv-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Volume(root)
    }

    fn path(&self, relative: &str) -> String {
        format!("{}/{relative}", self.0.display())
    }

    fn write(&self, full: &str, content: &[u8]) {
        let path = std::path::PathBuf::from(full);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// The live file.
    fn live(&self, relative: &str, content: &[u8]) {
        self.write(&self.path(relative), content);
    }

    /// The file as snapshot `id` had it.
    fn snap(&self, id: u64, relative: &str, content: &[u8]) {
        self.write(
            &self.path(&format!(".snapshots/{id}/snapshot/{relative}")),
            content,
        );
    }

    /// A snapshot that exists but does not hold the file.
    fn empty_snap(&self, id: u64) {
        fs::create_dir_all(self.path(&format!(".snapshots/{id}/snapshot"))).unwrap();
    }

    /// The history of `relative` over snapshots `ids`, through the real adapter code.
    fn history(&self, ids: &[u64], relative: &str) -> Result<Vec<(u64, FileState)>, BackendError> {
        let configs = format!(
            r#"{{"configs":[{{"config":"vol","subvolume":"{}"}}]}}"#,
            self.0.display()
        );
        let mut rows = vec![r#"{"number":0,"type":"single","date":"","user":"root","cleanup":"","description":"current"}"#.to_owned()];
        rows.extend(ids.iter().map(|n| {
                format!(r#"{{"number":{n},"type":"single","date":"2026-10-24 10:00:00","user":"root","cleanup":"number","description":"s{n}"}}"#)
            }));
        let list = format!(r#"{{"vol":[{}]}}"#, rows.join(","));
        let script = Scripted::new(vec![ok(&configs), ok(&list)]);
        SnapperCli::new(&script)
            .file_versions(&name("vol"), &self.path(relative))
            .map(|versions| {
                versions
                    .into_iter()
                    .map(|v| (v.snapshot.id.get(), v.state))
                    .collect()
            })
    }
}

impl Drop for Volume {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_is_compared_with_every_snapshot_by_content() {
    let v = Volume::new("states");
    v.live("docs/notes.txt", b"version two");
    v.snap(1, "docs/notes.txt", b"version two"); // identical
    v.snap(2, "docs/notes.txt", b"version one"); // same length, other bytes
    v.snap(3, "docs/notes.txt", b"v1"); // other length
    v.empty_snap(4); // the snapshot exists and the file was not in it
    assert_eq!(
        v.history(&[1, 2, 3, 4], "docs/notes.txt").unwrap(),
        [
            (1, FileState::Same),
            (2, FileState::Changed),
            (3, FileState::Changed),
            (4, FileState::AbsentThen),
        ]
    );
}

#[test]
fn a_deleted_file_is_deleted_since_where_it_existed_and_absent_where_it_did_not() {
    let v = Volume::new("deleted");
    v.snap(1, "gone.txt", b"it was here");
    v.empty_snap(2);
    assert_eq!(
        v.history(&[1, 2], "gone.txt").unwrap(),
        [(1, FileState::DeletedSince), (2, FileState::AbsentThen)]
    );
}

#[test]
fn large_files_are_compared_to_the_last_byte() {
    let v = Volume::new("large");
    let mut big = vec![7u8; 300 * 1024];
    v.live("big.bin", &big);
    v.snap(1, "big.bin", &big);
    *big.last_mut().unwrap() = 8;
    v.snap(2, "big.bin", &big); // equal length, differs only in the final byte
    assert_eq!(
        v.history(&[1, 2], "big.bin").unwrap(),
        [(1, FileState::Same), (2, FileState::Changed)]
    );
}

#[test]
fn permissions_count_because_restoring_puts_them_back() {
    let v = Volume::new("mode");
    v.live("run.sh", b"#!/bin/sh");
    v.snap(1, "run.sh", b"#!/bin/sh");
    fs::set_permissions(v.path("run.sh"), fs::Permissions::from_mode(0o644)).unwrap();
    fs::set_permissions(
        v.path(".snapshots/1/snapshot/run.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    assert_eq!(
        v.history(&[1], "run.sh").unwrap(),
        [(1, FileState::Changed)]
    );
}

#[test]
fn symlinks_are_compared_by_where_they_point_not_by_what_they_point_at() {
    let v = Volume::new("links");
    v.write(&v.path("target-a"), b"a");
    std::os::unix::fs::symlink("target-a", v.path("link")).unwrap();
    for id in [1, 2, 3] {
        fs::create_dir_all(v.path(&format!(".snapshots/{id}/snapshot"))).unwrap();
    }
    std::os::unix::fs::symlink("target-a", v.path(".snapshots/1/snapshot/link")).unwrap();
    std::os::unix::fs::symlink("target-b", v.path(".snapshots/2/snapshot/link")).unwrap();
    v.snap(3, "link", b"a regular file where the link is now");
    assert_eq!(
        v.history(&[1, 2, 3], "link").unwrap(),
        [
            (1, FileState::Same),
            (2, FileState::Changed),
            (3, FileState::Changed)
        ]
    );
}

#[test]
fn a_parent_that_is_a_file_in_the_snapshot_means_the_path_was_not_there() {
    let v = Volume::new("parent");
    v.live("a/b.txt", b"x");
    v.snap(1, "a", b"a was a file then, so a/b.txt could not exist");
    assert_eq!(
        v.history(&[1], "a/b.txt").unwrap(),
        [(1, FileState::AbsentThen)]
    );
}

#[test]
fn a_directory_is_refused_with_a_reason_not_compared_wrongly() {
    let v = Volume::new("dir");
    v.live("folder/inner.txt", b"x");
    v.snap(1, "folder/inner.txt", b"x");
    let error = v.history(&[1], "folder").unwrap_err();
    assert!(matches!(error, BackendError::NotSupported(_)), "{error}");
    assert!(error.to_string().contains("directories"), "{error}");
}

#[test]
fn a_snapshot_that_is_not_where_snapper_keeps_them_is_an_error_not_a_missing_file() {
    let v = Volume::new("nosnap");
    v.live("f.txt", b"x");
    // snapshot 1 is listed, but there is no .snapshots/1/snapshot at all
    let error = v.history(&[1], "f.txt").unwrap_err();
    assert!(
        error.to_string().contains("snapshot 1 is not at"),
        "{error}"
    );
}

// ---- which subvolume a file is on -----------------------------------------------------------

#[test]
fn a_file_is_covered_when_its_directory_is_on_the_configs_own_device() {
    let v = Volume::new("covers");
    v.live("docs/notes.txt", b"x");
    let covers = |path: &str| {
        let configs = format!(
            r#"{{"configs":[{{"config":"vol","subvolume":"{}"}}]}}"#,
            v.0.display()
        );
        let script = Scripted::new(vec![ok(&configs)]);
        SnapperCli::new(&script).covers(&name("vol"), path)
    };
    assert!(
        covers(&v.path("docs/notes.txt")).unwrap(),
        "a file in the subvolume"
    );
    assert!(
        covers(&v.path("docs/deleted-since.txt")).unwrap(),
        "a deleted file is where its directory is"
    );
    assert!(
        covers(&v.path("no/such/dir/at/all.txt")).unwrap(),
        "the nearest directory that exists decides"
    );
    // /proc is a different device, as a separate Btrfs subvolume is: not covered, though a path
    // under the subvolume by name would be
    assert!(
        !covers("/proc/self/environ").unwrap(),
        "another device is another subvolume"
    );
}

// ---- setting an area up ---------------------------------------------------------------------

#[test]
fn an_area_is_set_up_removed_and_adjusted_with_the_exact_commands() {
    let script = Scripted::new(vec![ok(""), ok(""), ok(""), ok("")]);
    let backend = SnapperCli::new(&script);
    backend.create_area(&name("home"), "/home").unwrap();
    backend
        .set_area_option(&name("home"), AreaOption::TimelineCreate(false))
        .unwrap();
    backend
        .set_area_option(&name("home"), AreaOption::TimelineCreate(true))
        .unwrap();
    backend.delete_area(&name("home")).unwrap();
    let calls = script.calls.borrow();
    assert_eq!(calls[0], ["-c", "home", "create-config", "/home"]);
    assert_eq!(calls[1], ["-c", "home", "set-config", "TIMELINE_CREATE=no"]);
    assert_eq!(
        calls[2],
        ["-c", "home", "set-config", "TIMELINE_CREATE=yes"]
    );
    assert_eq!(calls[3], ["-c", "home", "delete-config"]);
}

#[test]
fn snappers_own_words_come_back_when_setting_an_area_up_fails() {
    let script = Scripted::new(vec![failed(
        "Creating config failed (path is not a subvolume).",
    )]);
    let error = SnapperCli::new(&script)
        .create_area(&name("data"), "/data")
        .unwrap_err();
    assert!(
        error.to_string().contains("path is not a subvolume"),
        "{error}"
    );
}

// ---- space and retention ---------------------------------------------------------------------

#[test]
fn a_snapshots_measured_size_is_read_when_quotas_report_it_and_absent_when_they_do_not() {
    let listing = r#"{"home":[
            {"number":0,"type":"single","description":"current","userdata":null,"used-space":null},
            {"number":1,"type":"single","cleanup":"number","description":"with quotas","userdata":null,"used-space":3018752},
            {"number":2,"type":"single","cleanup":"number","description":"without","userdata":null,"used-space":null}]}"#;
    let script = Scripted::new(vec![ok(listing)]);
    let snapshots = SnapperCli::new(&script)
        .list_snapshots(&name("home"))
        .unwrap();
    assert_eq!(snapshots[0].used_bytes, Some(3_018_752));
    assert_eq!(snapshots[1].used_bytes, None, "not measured is not zero");
}

#[test]
fn retention_is_read_from_snappers_settings_and_what_it_does_not_report_stays_unknown() {
    let settings = r#"{"NUMBER_CLEANUP":"yes","NUMBER_LIMIT":"50","NUMBER_LIMIT_IMPORTANT":"10",
            "NUMBER_MIN_AGE":"1800","TIMELINE_CREATE":"no","SPACE_LIMIT":"0.5","QGROUP":""}"#;
    let script = Scripted::new(vec![ok(settings)]);
    let found = SnapperCli::new(&script).retention(&name("home")).unwrap();
    assert_eq!(
        script.calls.borrow()[0],
        ["--jsonout", "-c", "home", "get-config"]
    );
    assert_eq!(
        (found.keep, found.keep_important, found.min_age_seconds),
        (Some(50), Some(10), Some(1800))
    );
    assert!(found.cleanup && !found.on_a_schedule && !found.quotas);
    assert_eq!(found.space_limit_permille, Some(500));
    let empty = Retention::from_settings(&std::collections::BTreeMap::new());
    assert_eq!((empty.keep, empty.space_limit_permille), (None, None));
    assert!(!empty.cleanup, "an absent switch is not read as on");
}
