// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fitness functions: the dependency rule, checked against `cargo metadata` on every test
//! run. Documentation alone does not keep boundaries; this does.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn workspace_packages() -> Vec<serde_json::Value> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    metadata["packages"].as_array().expect("packages").clone()
}

/// Normal (non-dev, non-build) dependencies of a workspace package.
fn normal_deps(name: &str) -> BTreeSet<String> {
    let packages = workspace_packages();
    let package = packages
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("package {name} not found"));
    package["dependencies"]
        .as_array()
        .expect("dependencies")
        .iter()
        .filter(|d| d["kind"].is_null())
        .map(|d| d["name"].as_str().expect("name").to_owned())
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn core_depends_on_nothing_but_the_error_derive() {
    assert_eq!(normal_deps("snapall-core"), set(&["thiserror"]));
}

#[test]
fn the_snapper_adapter_depends_on_the_core_and_serialization_only() {
    assert!(normal_deps("snapall-snapper").is_subset(&set(&[
        "snapall-core",
        "serde",
        "serde_json"
    ])));
}

#[test]
fn the_observability_adapter_depends_on_the_core_and_serialization_only() {
    // It times operations and writes lines; it must not know any other adapter or any driver.
    let deps = normal_deps("snapall-observe");
    assert!(
        deps.is_subset(&set(&["snapall-core", "serde_json"])),
        "unexpected dependencies: {deps:?}"
    );
    assert!(deps.contains("snapall-core"));
}

#[test]
fn the_system_adapter_depends_on_the_core_alone() {
    // It asks the machine questions the core states; it needs nothing but the core's words for them.
    let deps = normal_deps("snapall-system");
    assert_eq!(
        deps,
        set(&["snapall-core"]),
        "unexpected dependencies: {deps:?}"
    );
}

#[test]
fn the_tui_depends_on_the_core_and_the_terminal_library_only() {
    let deps = normal_deps("snapall-tui");
    assert!(
        deps.is_subset(&set(&["snapall-core", "ratatui", "signal-hook"])),
        "unexpected dependencies: {deps:?}"
    );
    assert!(deps.contains("snapall-core"));
}

#[test]
fn drivers_and_adapters_never_know_each_other() {
    // The two driving adapters (CLI, TUI) and the driven adapter (snapper) meet only in the
    // composition root.
    for library in [
        "snapall-core",
        "snapall-snapper",
        "snapall-tui",
        "snapall-observe",
        "snapall-system",
    ] {
        let deps = normal_deps(library);
        for forbidden in [
            "snapall",
            "snapall-tui",
            "snapall-snapper",
            "snapall-observe",
            "snapall-system",
        ] {
            if forbidden != library {
                assert!(
                    !deps.contains(forbidden),
                    "{library} must not depend on {forbidden}"
                );
            }
        }
    }
}

#[test]
fn only_the_binary_may_know_argenv() {
    for library in [
        "snapall-core",
        "snapall-snapper",
        "snapall-tui",
        "snapall-observe",
        "snapall-system",
    ] {
        let deps = normal_deps(library);
        assert!(
            !deps.contains("argenv") && !deps.contains("argenv-resolve"),
            "{library} must not depend on argenv"
        );
    }
    assert!(normal_deps("snapall").contains("argenv-resolve"));
}

#[test]
fn nobody_depends_on_a_cli_parser_other_than_argenv() {
    for package in [
        "snapall-core",
        "snapall-snapper",
        "snapall-tui",
        "snapall-observe",
        "snapall-system",
        "snapall",
    ] {
        assert!(
            !normal_deps(package).contains("clap"),
            "{package} must not depend on clap: the invocation surface is declared with argenv"
        );
    }
}

/// The production code (comments and `#[cfg(test)]` modules removed) of every crate whose words a
/// person can read. The snapper adapter is left out: it is where the commands are actually run.
fn text_a_person_can_read() -> Vec<(String, String)> {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    for name in ["snapall", "snapall-core", "snapall-tui", "snapall-observe"] {
        let mut stack = vec![crates.join(name).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let source = std::fs::read_to_string(&path).unwrap();
                    let production = source.split("#[cfg(test)]").next().unwrap_or_default();
                    let code: String = production
                        .lines()
                        .filter(|line| !line.trim_start().starts_with("//"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    found.push((path.display().to_string(), code));
                }
            }
        }
    }
    found
}

/// A person is never told to run a raw `snapper` command: snapall does the setup, and where there is
/// something to run it is `snapall` itself. Found three times by hand before this existed.
#[test]
fn no_message_tells_a_person_to_run_a_raw_snapper_command() {
    for (file, code) in text_a_person_can_read() {
        for forbidden in ["snapper -c", "create-config", "set-config"] {
            assert!(
                !code.contains(forbidden),
                "`{forbidden}` in the production code of {file}: a person must never be sent outside snapall"
            );
        }
    }
}

/// The words snapall teaches are "protected area" and "snapshot". "snapper config" is the name of a
/// thing inside another program, and nobody reading snapall's output has been told what it is.
#[test]
fn no_message_calls_a_protected_area_a_snapper_config() {
    for (file, code) in text_a_person_can_read() {
        for forbidden in [
            "snapper config",
            "snapper's config",
            "snapper has no configs",
            "no configs",
        ] {
            assert!(
                !code.to_lowercase().contains(forbidden),
                "`{forbidden}` in the production code of {file}"
            );
        }
    }
}
