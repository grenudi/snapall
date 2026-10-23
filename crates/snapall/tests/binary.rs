//! The real binary, run as a subprocess. Kept to what needs no snapper: the invocation
//! surface and its published contract. Behaviour against a real snapper is tested by the
//! CI `integration` job.

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
