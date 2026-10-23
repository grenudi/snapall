// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fitness functions: the dependency rule, checked against `cargo metadata` on every test
//! run. Documentation alone does not keep boundaries; this does.

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
fn the_tui_depends_on_the_core_and_the_terminal_library_only() {
    let deps = normal_deps("snapall-tui");
    assert!(
        deps.is_subset(&set(&["snapall-core", "ratatui"])),
        "unexpected dependencies: {deps:?}"
    );
    assert!(deps.contains("snapall-core"));
}

#[test]
fn drivers_and_adapters_never_know_each_other() {
    // The two driving adapters (CLI, TUI) and the driven adapter (snapper) meet only in the
    // composition root.
    for library in ["snapall-core", "snapall-snapper", "snapall-tui"] {
        let deps = normal_deps(library);
        for forbidden in ["snapall", "snapall-tui", "snapall-snapper"] {
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
    for library in ["snapall-core", "snapall-snapper", "snapall-tui"] {
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
    for package in ["snapall-core", "snapall-snapper", "snapall-tui", "snapall"] {
        assert!(
            !normal_deps(package).contains("clap"),
            "{package} must not depend on clap: the invocation surface is declared with argenv"
        );
    }
}
