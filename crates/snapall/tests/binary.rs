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
