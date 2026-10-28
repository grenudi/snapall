// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the command line says, as text and as JSON.

#![cfg(test)]

use serde_json::{json, Value};
use snapall_core::protecting::doctor::OsRelease;
use snapall_core::{AreaName, Description};

use super::*;
use snapall_core::ports::testing::InMemoryBackend;
use snapall_core::{all_snapshots, plan_snapshots, snapshot_all, SnapshotBackend};

#[test]
fn report_lists_successes_and_failures_aligned() {
    let backend = InMemoryBackend::with_areas(&["root", "home"]).failing_on("home");
    let r = snapshot_all(&backend, None, &Description::default(), None).unwrap();
    assert_eq!(
            report(&r),
            "FAIL  home  `in-memory` failed (injected failure): snapshot of home refused\nok    root  #1\n"
        );
    let value = report_json(&r);
    assert_eq!(value["ok"], false);
    assert_eq!(value["outcomes"][1]["snapshot"], 1);
    assert!(value["outcomes"][0]["error"]
        .as_str()
        .unwrap()
        .contains("refused"));
}

#[test]
fn dry_run_names_every_target_in_both_formats() {
    let backend = InMemoryBackend::with_areas(&["root", "home"]);
    let plan = plan_snapshots(&backend, None).unwrap();
    let description = Description::parse("x").unwrap();
    let text = dry_run(&plan, &description);
    assert!(text.contains("would snapshot 2 area(s)"));
    assert!(text.contains("  home\n") && text.contains("  root\n"));
    let value = dry_run_json(&plan, &description);
    assert_eq!(value["would_snapshot"], json!(["home", "root"]));
    assert_eq!(value["dry_run"], true);
}

#[test]
fn areas_are_sorted_in_both_formats() {
    let names = [
        AreaName::parse("root").unwrap(),
        AreaName::parse("home").unwrap(),
    ];
    assert_eq!(protected_areas(&names), "home\nroot\n");
    assert_eq!(areas_json(&names), json!({"configs": ["home", "root"]}));
}

#[test]
fn snapshot_tables_align_and_empty_areas_say_so() {
    let backend = InMemoryBackend::with_areas(&["home", "root"]);
    let home = AreaName::parse("home").unwrap();
    for text in ["before theming", "after theming"] {
        backend
            .create_snapshot(&home, &Description::parse(text).unwrap())
            .unwrap();
    }
    let all = all_snapshots(&backend).unwrap();
    let text = snapshots(&all);
    assert_eq!(
        text,
        "home\n  \
             #  type    date                 cleanup  description\n  \
             1  single  2026-01-01 00:00:01  number   before theming\n  \
             2  single  2026-01-01 00:00:02  number   after theming\n\
             \nroot\n  (no snapshots)\n"
    );
    let value = snapshots_json(&all);
    assert_eq!(value["configs"]["home"][1]["description"], "after theming");
    assert_eq!(value["configs"]["home"][0]["type"], "single");
    assert_eq!(value["configs"]["root"], json!([]));
}

#[test]
fn doctor_output_shows_status_detail_and_fix() {
    use snapall_core::protecting::doctor::{diagnose, Probe};
    let probe = Probe {
        os: OsRelease::parse("ID=arch\nPRETTY_NAME=\"Arch Linux\"\n"),
        snapper: Err("No such file or directory".into()),
        is_root: Some(false),
        areas: None,
        locations: Vec::new(),
    };
    let checks = diagnose(&probe);
    let text = doctor(&probe.os, &checks);
    assert!(text.starts_with("system: Arch Linux\n"), "{text}");
    assert!(text.contains("FAIL  snapper"), "{text}");
    assert!(
        text.contains("fix: pacman -S --needed --noconfirm snapper"),
        "{text}"
    );
    assert!(text.contains("warn  privileges"), "{text}");
    let value = doctor_json(&probe.os, &checks);
    assert_eq!(value["ok"], false);
    assert_eq!(value["checks"][0]["status"], "fail");
    assert_eq!(value["checks"][1]["fix"], "run with sudo");
}

// ---- restore points ------------------------------------------------------------------------

fn points_fixture() -> (
    snapall_core::ports::testing::InMemoryBackend,
    snapall_core::RestorePoint,
) {
    use snapall_core::ports::testing::InMemoryBackend;
    let backend = InMemoryBackend::with_areas(&["docs", "home", "root"]).with_root_area("root");
    backend.write_file("docs", "/mnt/docs/a.txt", "then");
    backend.write_file("home", "/mnt/home/b.txt", "then");
    let plan = snapall_core::plan_snapshots(&backend, None).unwrap();
    let point = snapall_core::Point::parse("20261025T100000Z").unwrap();
    snapall_core::run_plan(
        &backend,
        &plan,
        &Description::parse("before theming").unwrap(),
        Some(&point),
    );
    let found = snapall_core::restore_points(&snapall_core::all_snapshots(&backend).unwrap());
    (backend, found.into_iter().next().unwrap())
}

#[test]
fn no_restore_points_says_what_makes_one() {
    let text = points(&[]);
    assert!(
        text.contains("No restore points yet") && text.contains("sudo snapall"),
        "{text}"
    );
    assert_eq!(points_json(&[]), json!({"points": []}));
}

#[test]
fn restore_points_are_a_table_with_the_command_that_uses_them() {
    let (_, point) = points_fixture();
    let text = points(std::slice::from_ref(&point));
    assert!(text.contains("sudo snapall restore-all POINT"), "{text}");
    assert!(
        text.contains("POINT")
            && text.contains("WHEN")
            && text.contains("AREAS")
            && text.contains("NAME"),
        "{text}"
    );
    assert!(
        text.contains("20261025T100000Z")
            && text.contains("docs, home, root")
            && text.contains("before theming"),
        "{text}"
    );
    assert!(
        !text.contains("matched"),
        "a tagged point is not marked as a guess: {text}"
    );
    let value = points_json(std::slice::from_ref(&point));
    assert_eq!(value["points"][0]["point"], "20261025T100000Z");
    assert_eq!(value["points"][0]["tagged"], true);
    assert_eq!(
        value["points"][0]["areas"][1],
        json!({"area": "home", "snapshot": 1})
    );
}

#[test]
fn a_matched_point_is_marked_as_older_and_a_guess() {
    let (_, mut point) = points_fixture();
    point.tagged = false;
    assert!(points(&[point]).contains("older: matched by name and time"));
}

#[test]
fn the_preview_says_per_area_what_changes_and_why_the_system_is_left_out() {
    let (backend, point) = points_fixture();
    backend.write_file("home", "/mnt/home/b.txt", "ruined");
    let text = point_preview(
        &point,
        &snapall_core::preview_point(&backend, &point),
        "boot from a live USB",
    );
    assert!(
        text.contains("docs") && text.contains("nothing to change: it already matches"),
        "{text}"
    );
    assert!(
        text.contains("change 1 path: 1 reverted, 0 removed, 0 brought back"),
        "{text}"
    );
    assert!(
        text.contains("not touched: the running system is not put back while it runs"),
        "{text}"
    );
    assert!(
        text.contains("The system itself: boot from a live USB"),
        "{text}"
    );
    let value = point_preview_json(&point, &snapall_core::preview_point(&backend, &point));
    let actions: Vec<&str> = value["areas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["action"].as_str().unwrap())
        .collect();
    assert_eq!(actions, ["unchanged", "change", "system"]);
}

#[test]
fn what_was_done_says_each_area_and_the_one_command_that_undoes_all_of_it_only_if_something_changed(
) {
    let (backend, point) = points_fixture();
    backend.write_file("home", "/mnt/home/b.txt", "ruined");
    let done = snapall_core::restore_point(
        &backend,
        &point,
        snapall_core::Point::parse("20261025T110000Z").unwrap(),
    );
    let text = point_restored(&point, &done);
    assert!(
        text.contains("put back (1 path)")
            && text.contains("already matched: left alone")
            && text.contains("left alone: the running system"),
        "{text}"
    );
    assert!(
        text.contains("To undo all of this at once: sudo snapall restore-all 20261025T110000Z"),
        "{text}"
    );
    assert_eq!(
        point_restored_json(&point, &done)["undo_point"],
        "20261025T110000Z"
    );
    // nothing differs now, so nothing is done and there is nothing to undo
    let nothing = snapall_core::restore_point(
        &backend,
        &point,
        snapall_core::Point::parse("20261025T120000Z").unwrap(),
    );
    let text = point_restored(&point, &nothing);
    assert!(
        text.contains("Nothing was changed.") && !text.contains("To undo"),
        "{text}"
    );
    assert_eq!(
        point_restored_json(&point, &nothing)["undo_point"],
        Value::Null
    );
}
