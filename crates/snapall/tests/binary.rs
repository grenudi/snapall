// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The real binary, run as a subprocess. Kept to what needs no snapper: the invocation
//! surface and its published contract. Behaviour against a real snapper is tested by the
//! CI `integration` job.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn snapall(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_snapall"))
        .args(args)
        // A clean environment: nothing from the developer's shell may change the result.
        .env_clear()
        .output()
        .expect("run snapall")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn version_and_help_succeed() {
    let version = snapall(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        stdout(&version).trim(),
        concat!("snapall ", env!("CARGO_PKG_VERSION"))
    );

    let help = snapall(&["--help"]);
    assert!(help.status.success());
    let text = stdout(&help);
    for needle in [
        "USAGE",
        "COMMANDS",
        "--dry-run",
        "SNAPALL_ONLY",
        "EXIT STATUS",
    ] {
        assert!(text.contains(needle), "help is missing `{needle}`:\n{text}");
    }
}

#[test]
fn a_wrong_invocation_exits_2_with_the_reason_on_stderr_and_nothing_on_stdout() {
    for args in [
        &["--evil"][..],
        &["lst"],
        &["delete", "home", "3", "--dry-run"],
        &["list", "extra"],
    ] {
        let output = snapall(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(stderr(&output).contains("snapall: error:"), "{args:?}");
        assert!(stdout(&output).is_empty(), "{args:?}");
    }
}

#[test]
fn a_missing_snapper_is_a_runtime_failure_not_a_usage_error() {
    // An explicitly empty PATH, so the result does not depend on whether this machine has
    // snapper installed (an unset PATH would fall back to the system defaults).
    let output = Command::new(env!("CARGO_BIN_EXE_snapall"))
        .arg("list")
        .env_clear()
        .env("PATH", "/nonexistent")
        .output()
        .expect("run snapall");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("could not run `snapper`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn tui_refuses_to_run_without_a_terminal() {
    let output = snapall(&["tui"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("interactive terminal"),
        "{}",
        stderr(&output)
    );
}

/// The contract document with the volatile fields removed.
fn stable(mut document: Value) -> Value {
    let object = document.as_object_mut().expect("document is an object");
    object.remove("generated");
    object.remove("source");
    document
}

#[test]
fn the_contract_command_emits_a_valid_envelope() {
    let output = snapall(&["contract"]);
    assert!(output.status.success());
    let document: Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(document["contract_version"], 1);
    assert_eq!(
        document["source"],
        concat!("snapall@", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        document["precedence"],
        serde_json::json!(["arg", "env", "default"])
    );
    assert!(document["inputs"].as_array().is_some_and(|i| i.len() >= 9));
}

#[test]
fn the_committed_contract_is_the_one_the_code_produces() {
    let committed = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../snapall.argenv.json");
    let committed: Value = serde_json::from_str(
        &std::fs::read_to_string(committed).expect("snapall.argenv.json is committed"),
    )
    .expect("committed contract is JSON");
    let produced: Value =
        serde_json::from_str(&stdout(&snapall(&["contract"]))).expect("valid JSON");
    assert_eq!(
        stable(committed),
        stable(produced),
        "snapall.argenv.json is stale: run `cargo run -q -p snapall -- contract > snapall.argenv.json`"
    );
}

#[test]
fn a_reader_that_goes_away_is_not_an_error() {
    // `snapall contract | head -0`: the reader closes before the output is written. That must
    // end quietly, not with Rust's "failed printing to stdout: Broken pipe" panic.
    let mut child = Command::new(env!("CARGO_BIN_EXE_snapall"))
        .arg("contract")
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn snapall");
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("wait");
    assert!(
        !stderr(&output).contains("panicked"),
        "snapall panicked on a closed pipe:\n{}",
        stderr(&output)
    );
    assert!(output.status.success(), "{:?}", output.status);
}

/// A snapall run whose `snapper` cannot be found (an empty PATH): deterministic, and it still
/// reaches the backend, so there is an operation to observe.
fn snapall_without_snapper(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_snapall"))
        .args(args)
        .env_clear()
        .env("PATH", "/nonexistent")
        .output()
        .expect("run snapall")
}

#[test]
fn logging_to_stderr_reports_the_failed_operation_as_one_json_line() {
    let output = snapall_without_snapper(&["--log", "json", "list"]);
    assert_eq!(output.status.code(), Some(1));
    let log: Vec<Value> = stderr(&output)
        .lines()
        .filter(|l| l.starts_with('{'))
        .map(|l| serde_json::from_str(l).expect("every log line is JSON"))
        .collect();
    assert_eq!(log.len(), 1, "{}", stderr(&output));
    assert_eq!(log[0]["event"], "configs_listed");
    assert_eq!(log[0]["level"], "error");
    assert!(log[0]["error"]
        .as_str()
        .is_some_and(|e| e.contains("snapper")));
    assert!(log[0]["time"].as_str().is_some_and(|t| t.ends_with('Z')));
}

#[test]
fn a_log_file_is_private_appended_to_and_kept_out_of_stderr() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!("snapall-log-test-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let file = path.to_str().expect("utf-8 temp path");

    for _ in 0..2 {
        let output = snapall_without_snapper(&["--log-file", file, "list"]);
        assert!(
            !stderr(&output).contains("configs_listed"),
            "events must go to the file, not stderr: {}",
            stderr(&output)
        );
    }
    let text = std::fs::read_to_string(&path).expect("the log file exists");
    assert_eq!(text.lines().count(), 2, "the second run appended: {text}");
    for line in text.lines() {
        let value: Value = serde_json::from_str(line).expect("JSON line");
        assert_eq!(value["event"], "configs_listed");
    }
    let mode = std::fs::metadata(&path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "a log of privileged actions is private");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn without_logging_nothing_extra_is_written() {
    let output = snapall_without_snapper(&["list"]);
    assert!(
        !stderr(&output).contains("configs_listed"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_log_file_that_cannot_be_opened_is_a_clear_error_not_a_silent_loss() {
    let output = snapall_without_snapper(&["--log-file", "/nonexistent-dir/x.log", "list"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("cannot open the log file"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn completions_for_every_shell_come_from_the_contract() {
    for (shell, needle) in [
        ("bash", "complete -F _snapall snapall"),
        ("zsh", "#compdef snapall"),
        ("fish", "complete -c snapall"),
        ("nushell", "export extern \"snapall create\""),
    ] {
        let output = snapall(&["completions", shell]);
        assert!(output.status.success(), "{shell}: {}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains(needle), "{shell} lacks `{needle}`");
        // what the user can type is what the contract says
        // names, not spellings: fish writes `-l dry-run`, bash `--dry-run`
        for word in [
            "dry-run",
            "only",
            "log-file",
            "doctor",
            "snapshots",
            "completions",
        ] {
            assert!(text.contains(word), "{shell} lacks `{word}`");
        }
    }
}

#[test]
fn a_flag_that_belongs_to_one_command_is_not_offered_for_another_in_the_real_script() {
    let bash = stdout(&snapall(&["completions", "bash"]));
    let line = |command: &str| {
        bash.lines()
            .find(|l| l.trim_start().starts_with(&format!("{command}) flags=")))
            .unwrap_or_else(|| panic!("no flags line for {command}"))
            .to_owned()
    };
    assert!(line("create").contains("--dry-run") && line("create").contains("--only"));
    assert!(
        !line("delete").contains("--dry-run"),
        "the very mistake `lint` rejects"
    );
    assert!(!line("list").contains("--only"));
    assert!(line("doctor").contains("--install"));
}

#[test]
fn the_man_page_is_reproducible_and_dated_by_source_date_epoch() {
    let render = || {
        Command::new(env!("CARGO_BIN_EXE_snapall"))
            .arg("man")
            .env_clear()
            .env("SOURCE_DATE_EPOCH", "1761187392")
            .output()
            .expect("run snapall man")
    };
    let (first, second) = (render(), render());
    assert!(first.status.success(), "{}", stderr(&first));
    assert_eq!(stdout(&first), stdout(&second), "byte for byte");
    let text = stdout(&first);
    assert!(
        text.starts_with(".TH SNAPALL 1 \"2025-10-23\" \"snapall "),
        "{}",
        &text[..60]
    );
    for section in [
        "NAME",
        "SYNOPSIS",
        "COMMANDS",
        "OPTIONS",
        "ARGUMENTS",
        "ENVIRONMENT",
        "EXIT STATUS",
        "SEE ALSO",
    ] {
        assert!(text.contains(&format!(".SH {section}")), "{section}");
    }
    assert!(text.contains("snapper(8)") && text.contains("SNAPALL_ONLY"));
}

// ---- restore, through the real binary against a fake `snapper` -------------------------------
//
// A shell script stands in for snapper, answering exactly what the real one was observed to print
// (see crates/snapall-snapper). That lets the binary be driven through cases a throwaway Btrfs
// volume cannot produce, such as a config whose subvolume is the system root `/`.

/// A fake snapper with two configs: `home` at /home and `root` at `/`, each with snapshot #1.
/// `status` reports three changes; `undochange` and `create` succeed and log what they were asked.
fn fake_snapper(label: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!(
        "snapall-fake-snapper-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create dir");
    let script = r#"#!/bin/sh
echo "$@" >> "$0.log"
case "$*" in
  *list-configs*) echo '{"configs":[{"config":"home","subvolume":"/home"},{"config":"root","subvolume":"/"}]}' ;;
  *" list"*) echo '{"'"$(echo "$*" | sed 's/.*-c \([a-z]*\) .*/\1/')"'":[{"number":0,"type":"single","date":"","user":"root","cleanup":"","description":"current"},{"number":1,"type":"single","date":"2026-10-24 10:00:00","user":"root","cleanup":"number","description":"base"}]}' ;;
  *" status 1..0"*) printf '%s\n' '+..... /home/u/new.txt' '-..... /home/u/old.txt' 'c..... /home/u/edit.txt' ;;
  *" create "*) echo 5 ;;
  *undochange*) echo 'create:1 modify:1 delete:1' ;;
  *) echo "fake snapper: unhandled: $*" >&2; exit 3 ;;
esac
"#;
    let path = dir.join("snapper");
    std::fs::write(&path, script).expect("write script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    dir
}

fn with_fake(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_snapall"))
        .args(args)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", dir.display()))
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run snapall")
}

#[test]
fn changes_lists_what_restoring_would_do_in_words_and_in_json() {
    let dir = fake_snapper("changes");
    let human = with_fake(&dir, &["--no-dbus", "changes", "home", "1"]);
    assert!(human.status.success(), "{}", stderr(&human));
    let text = stdout(&human);
    assert!(text.contains("restoring #1 would change 3 paths"), "{text}");
    assert!(
        text.contains("remove") && text.contains("/home/u/new.txt"),
        "{text}"
    );
    assert!(
        text.contains("bring back") && text.contains("/home/u/old.txt"),
        "{text}"
    );
    assert!(
        text.contains("revert") && text.contains("/home/u/edit.txt"),
        "{text}"
    );

    let json = with_fake(&dir, &["--no-dbus", "changes", "home", "1", "--json"]);
    let value: Value = serde_json::from_str(&stdout(&json)).expect("JSON");
    assert_eq!(value["config"], "home");
    assert_eq!(value["snapshot"], 1);
    assert_eq!(value["system_root"], false);
    assert_eq!(value["changes"].as_array().unwrap().len(), 3);
    assert_eq!(
        value["changes"][0],
        serde_json::json!({"kind": "added", "path": "/home/u/new.txt"})
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restoring_the_system_root_is_refused_with_the_way_that_works_and_nothing_is_run() {
    let dir = fake_snapper("root");
    let output = with_fake(&dir, &["--no-dbus", "restore", "root", "1", "--yes"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = stderr(&output);
    assert!(message.contains("is the system root"), "{message}");
    assert!(
        message.contains("live USB") || message.contains("boot menu"),
        "the supported route is named: {message}"
    );
    let log = std::fs::read_to_string(dir.join("snapper.log")).unwrap_or_default();
    assert!(
        !log.contains("undochange") && !log.contains(" create "),
        "nothing was changed or snapshotted:\n{log}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_single_path_of_the_system_root_may_be_restored() {
    let dir = fake_snapper("root-path");
    // A path of the root where something differs is restored...
    let output = with_fake(
        &dir,
        &[
            "--no-dbus",
            "restore",
            "root",
            "1",
            "/home/u/edit.txt",
            "--yes",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let log = std::fs::read_to_string(dir.join("snapper.log")).unwrap();
    assert!(log.contains("undochange 1..0 /home/u/edit.txt"), "{log}");

    // ...and a path where nothing differs is not, and wastes no safety snapshot.
    let nothing = with_fake(
        &dir,
        &["--no-dbus", "restore", "root", "1", "/etc/fstab", "--yes"],
    );
    assert!(nothing.status.success());
    assert!(
        stdout(&nothing).contains("nothing would change"),
        "{}",
        stdout(&nothing)
    );
    let log = std::fs::read_to_string(dir.join("snapper.log")).unwrap();
    assert!(!log.contains("/etc/fstab"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_without_a_terminal_and_without_yes_refuses_and_changes_nothing() {
    let dir = fake_snapper("noyes");
    let output = with_fake(&dir, &["--no-dbus", "restore", "home", "1"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a usage problem: {}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("--yes"), "{}", stderr(&output));
    let log = std::fs::read_to_string(dir.join("snapper.log")).unwrap_or_default();
    assert!(
        !log.contains("undochange") && !log.contains(" create "),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_takes_the_safety_snapshot_first_then_restores_and_reports_both() {
    let dir = fake_snapper("restore");
    let output = with_fake(
        &dir,
        &["--no-dbus", "restore", "home", "1", "--yes", "--json"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let value: Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    assert_eq!(value["safety_snapshot"], 5);
    assert_eq!(
        (
            value["reverted"].clone(),
            value["removed"].clone(),
            value["brought_back"].clone()
        ),
        (1.into(), 1.into(), 1.into())
    );
    let log = std::fs::read_to_string(dir.join("snapper.log")).unwrap();
    let create = log.find(" create ").expect("a safety snapshot was taken");
    let undo = log.find("undochange").expect("then the restore ran");
    assert!(create < undo, "the safety snapshot must come first:\n{log}");
    assert!(log.contains("before restoring #1"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dry_run_restore_changes_nothing_and_a_restore_to_snapshot_zero_is_a_usage_error() {
    let dir = fake_snapper("dry");
    let output = with_fake(&dir, &["--no-dbus", "restore", "home", "1", "--dry-run"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("would change 3 paths"));
    let log = std::fs::read_to_string(dir.join("snapper.log")).unwrap_or_default();
    assert!(
        !log.contains("undochange") && !log.contains(" create "),
        "{log}"
    );
    let zero = with_fake(&dir, &["--no-dbus", "restore", "home", "0", "--yes"]);
    assert_eq!(zero.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- a file's history, and the audit trail of a restore ---------------------------------------

/// A real directory laid out the way snapper lays out snapshots, behind a fake `snapper` that says
/// it is the subvolume of a config called `vol`: notes.txt is "now" live, snapshot 1 had other
/// text, snapshot 2 had the same, snapshot 3 did not have the file.
struct Volume {
    root: std::path::PathBuf,
    bin: std::path::PathBuf,
}

impl Volume {
    fn new(label: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("snapall-vol-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let put = |relative: &str, text: &str| {
            let path = root.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        put("docs/notes.txt", "the text now");
        put(".snapshots/1/snapshot/docs/notes.txt", "older text");
        put(".snapshots/2/snapshot/docs/notes.txt", "the text now");
        std::fs::create_dir_all(root.join(".snapshots/3/snapshot/docs")).unwrap();
        let bin =
            std::env::temp_dir().join(format!("snapall-vol-bin-{label}-{}", std::process::id()));
        std::fs::create_dir_all(&bin).unwrap();
        let script = format!(
            r#"#!/bin/sh
echo "$@" >> "$0.log"
case "$*" in
  *list-configs*) echo '{{"configs":[{{"config":"vol","subvolume":"{root}"}}]}}' ;;
  *"-c vol list"*) echo '{{"vol":[
    {{"number":0,"type":"single","date":"","user":"root","cleanup":"","description":"current"}},
    {{"number":1,"type":"single","date":"2026-10-24 10:00:00","user":"root","cleanup":"number","description":"older"}},
    {{"number":2,"type":"single","date":"2026-10-24 11:00:00","user":"root","cleanup":"number","description":"same"}},
    {{"number":3,"type":"single","date":"2026-10-24 12:00:00","user":"root","cleanup":"number","description":"before the file"}}]}}' ;;
  *) echo "fake snapper: unhandled: $*" >&2; exit 3 ;;
esac
"#,
            root = root.display()
        );
        let path = bin.join("snapper");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Volume { root, bin }
    }

    fn run(&self, cwd: &std::path::Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_snapall"))
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .stdin(std::process::Stdio::null())
            .output()
            .expect("run snapall")
    }
}

impl Drop for Volume {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        let _ = std::fs::remove_dir_all(&self.bin);
    }
}

#[test]
fn history_says_how_the_file_stood_in_each_snapshot() {
    let v = Volume::new("human");
    let file = v.root.join("docs/notes.txt");
    let output = v.run(&v.root, &["--no-dbus", "history", file.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("(config vol)"), "{text}");
    let line = |id: &str| {
        text.lines()
            .find(|l| l.trim_start().starts_with(id))
            .unwrap_or_else(|| panic!("{text}"))
            .to_owned()
    };
    assert!(line("1 ").contains("differs from now"), "{text}");
    assert!(line("2 ").contains("same as now"), "{text}");
    assert!(line("3 ").contains("did not exist"), "{text}");
}

#[test]
fn history_as_json_has_stable_tokens_and_says_what_can_be_restored() {
    let v = Volume::new("json");
    let output = v.run(
        &v.root.join("docs"),
        &["--no-dbus", "history", "notes.txt", "--json"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let value: Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    assert_eq!(value["config"], "vol");
    assert!(
        value["path"].as_str().unwrap().ends_with("/docs/notes.txt"),
        "a relative path is made absolute: {value}"
    );
    let versions = value["versions"].as_array().unwrap();
    let states: Vec<_> = versions
        .iter()
        .map(|v| {
            (
                v["snapshot"].as_u64().unwrap(),
                v["state"].as_str().unwrap().to_owned(),
                v["restorable"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        states,
        [
            (1, "changed".to_owned(), true),
            (2, "same".to_owned(), false),
            (3, "absent_then".to_owned(), false)
        ]
    );
}

#[test]
fn history_refuses_a_directory_and_a_file_no_config_covers_with_the_reason() {
    let v = Volume::new("errors");
    let dir = v.run(
        &v.root,
        &[
            "--no-dbus",
            "history",
            v.root.join("docs").to_str().unwrap(),
        ],
    );
    assert_eq!(dir.status.code(), Some(1));
    assert!(stderr(&dir).contains("directories"), "{}", stderr(&dir));
    let outside = v.run(&v.root, &["--no-dbus", "history", "/etc/hostname"]);
    assert_eq!(outside.status.code(), Some(1));
    assert!(
        stderr(&outside).contains("No snapshot contains /etc/hostname"),
        "{}",
        stderr(&outside)
    );
    assert!(
        !stderr(&outside).contains("snapper config"),
        "no jargon: {}",
        stderr(&outside)
    );
}

#[test]
fn the_tui_opened_on_a_file_still_needs_a_terminal() {
    let v = Volume::new("tui");
    let output = v.run(&v.root, &["--no-dbus", "tui", "--file", "docs/notes.txt"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("interactive terminal"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_restore_can_be_logged_and_the_log_says_what_it_restored() {
    // 0.5.0 rejected --log on restore, so the most consequential command had no audit trail.
    let dir = fake_snapper("audit");
    let output = with_fake(
        &dir,
        &[
            "--no-dbus",
            "restore",
            "home",
            "1",
            "--yes",
            "--log",
            "json",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let log: Vec<Value> = stderr(&output)
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let undone = log
        .iter()
        .find(|e| e["event"] == "changes_undone")
        .expect("a changes_undone line");
    assert_eq!(undone["config"], "home");
    assert_eq!(undone["snapshot"], 1);
    assert_eq!(undone["modified"], 1);
    assert!(
        log.iter().any(|e| e["event"] == "snapshot_created"),
        "the safety snapshot is logged too"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_no_snapshot_can_contain_says_so_in_plain_words_and_never_claims_it_did_not_exist() {
    // The user's report: a file in /home, where /home is a separate subvolume and only a config for
    // `/` exists. /proc stands in for the separate subvolume (another device, under `/` by name).
    let dir = fake_snapper("othervol");
    let output = with_fake(&dir, &["--no-dbus", "history", "/proc/self/environ"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = stderr(&output);
    assert!(message.contains("No snapshot contains /proc/"), "{message}");
    assert!(
        message.contains("cannot protect"),
        "/proc is not a disk snapall can protect: {message}"
    );
    for jargon in ["snapper", "subvolume", "create-config"] {
        assert!(!message.contains(jargon), "`{jargon}` in: {message}");
    }
    assert!(
        !stdout(&output).contains("did not exist"),
        "it must not claim the file did not exist"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
