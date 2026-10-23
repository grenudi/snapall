// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The port contract, run against a real snapper on a real Btrfs volume.
//!
//! Skipped unless `SNAPALL_IT=1`. CI sets it up with `dev/btrfs-fixture.sh up` (root,
//! loop devices, snapper and btrfs-progs required).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use snapall_core::testing::backend_contract;
use snapall_core::{snapshot_all, ConfigName, Description};
use snapall_snapper::SnapperCli;

fn enabled() -> bool {
    if std::env::var_os("SNAPALL_IT").is_none() {
        eprintln!("skipped: set SNAPALL_IT=1 (see dev/btrfs-fixture.sh)");
        return false;
    }
    true
}

fn backend() -> SnapperCli {
    SnapperCli::system().with_no_dbus(true)
}

#[test]
fn contract_holds_against_real_snapper() {
    if !enabled() {
        return;
    }
    let config = ConfigName::parse("snapall_it_a").unwrap();
    backend_contract(&backend(), &config);
}

#[test]
fn snapshot_all_covers_every_fixture_config() {
    if !enabled() {
        return;
    }
    let report = snapshot_all(
        &backend(),
        None,
        &Description::parse("it: snapshot_all").unwrap(),
    )
    .expect("plan and run");
    // Other configs may exist on the machine; assert on the fixture's own.
    for name in ["snapall_it_a", "snapall_it_b"] {
        let outcome = report
            .outcomes
            .iter()
            .find(|o| o.config.as_str() == name)
            .unwrap_or_else(|| panic!("{name} missing from {report:?}"));
        assert!(outcome.result.is_ok(), "{name}: {:?}", outcome.result);
    }
}
