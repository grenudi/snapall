// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The doctor, from facts about a machine to findings.

#![cfg(test)]

use crate::vocabulary::domain::AreaName;

use super::*;

fn os(text: &str) -> OsRelease {
    OsRelease::parse(text)
}

fn steps(plan: InstallPlan) -> Vec<String> {
    match plan {
        InstallPlan::Steps(steps) => steps.iter().map(Step::display).collect(),
        InstallPlan::Manual(text) => panic!("expected steps, got manual: {text}"),
    }
}

fn manual(plan: InstallPlan) -> String {
    match plan {
        InstallPlan::Manual(text) => text,
        InstallPlan::Steps(steps) => panic!("expected manual, got {steps:?}"),
    }
}

// Real /etc/os-release contents, trimmed to the lines that matter.
const DEBIAN: &str = "PRETTY_NAME=\"Debian GNU/Linux 13 (trixie)\"\nID=debian\n";
const UBUNTU: &str = "PRETTY_NAME=\"Ubuntu 24.04.3 LTS\"\nID=ubuntu\nID_LIKE=debian\n";
const MINT: &str = "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n";
const FEDORA: &str = "ID=fedora\nVARIANT_ID=workstation\nPRETTY_NAME=\"Fedora Linux 43\"\n";
const SILVERBLUE: &str =
    "ID=fedora\nVARIANT_ID=silverblue\nPRETTY_NAME=\"Fedora Linux 43 (Silverblue)\"\n";
const ROCKY: &str =
    "ID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\nPRETTY_NAME=\"Rocky Linux 10\"\n";
const NOBARA: &str = "ID=nobara\nID_LIKE=\"rhel centos fedora\"\n";
const ARCH: &str = "NAME=\"Arch Linux\"\nID=arch\n";
const CACHYOS: &str = "NAME=\"CachyOS Linux\"\nID=cachyos\nID_LIKE=arch\n";
const MANJARO: &str = "ID=manjaro\nID_LIKE=arch\n";
const TUMBLEWEED: &str = "ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n";
const LEAP: &str = "ID=\"opensuse-leap\"\nID_LIKE=\"suse opensuse\"\n";
const MICROOS: &str = "ID=\"opensuse-microos\"\nID_LIKE=\"suse opensuse opensuse-tumbleweed\"\n";
const ALPINE: &str = "ID=alpine\nPRETTY_NAME=\"Alpine Linux v3.22\"\n";
const ALPINE_EDGE: &str = "ID=alpine\nPRETTY_NAME=\"Alpine Linux edge\"\n";
const VOID: &str = "ID=\"void\"\n";
const GENTOO: &str = "ID=gentoo\n";
const NIXOS: &str = "ID=nixos\n";
const STEAMOS: &str = "ID=steamos\nID_LIKE=arch\n";

#[test]
fn os_release_parsing_handles_quotes_comments_and_case() {
    let parsed = os("# a comment\n\nID=\"Rocky\"\nID_LIKE='rhel  centos'\nNAME=ignored\nbroken line\nPRETTY_NAME=\"Rocky Linux 10\"\n");
    assert_eq!(parsed.id, "rocky");
    assert_eq!(parsed.id_like, ["rhel", "centos"]);
    assert_eq!(parsed.pretty_name.as_deref(), Some("Rocky Linux 10"));
    assert_eq!(OsRelease::parse(""), OsRelease::default());
    assert_eq!(OsRelease::parse("garbage \0 \u{1F980}").id, "");
}

#[test]
fn every_major_family_is_recognised_by_its_own_id_or_its_parents() {
    for (text, expected) in [
        (DEBIAN, Family::Debian),
        (UBUNTU, Family::Debian),
        (MINT, Family::Debian),
        (FEDORA, Family::Fedora),
        (ROCKY, Family::Rhel),
        (NOBARA, Family::Fedora),
        (ARCH, Family::Arch),
        (CACHYOS, Family::Arch),
        (MANJARO, Family::Arch),
        (TUMBLEWEED, Family::Suse),
        (LEAP, Family::Suse),
        (ALPINE, Family::Alpine),
        (VOID, Family::Void),
        (GENTOO, Family::Gentoo),
        (NIXOS, Family::Nix),
        ("ID=plan9\n", Family::Unknown),
        ("", Family::Unknown),
    ] {
        assert_eq!(family(&os(text)), expected, "{text}");
    }
}

#[test]
fn install_plans_use_the_package_managers_real_syntax() {
    assert_eq!(
        steps(install_plan(&os(DEBIAN))),
        ["apt-get update", "apt-get install -y snapper"]
    );
    assert_eq!(
        steps(install_plan(&os(UBUNTU))),
        ["apt-get update", "apt-get install -y snapper"]
    );
    assert_eq!(
        steps(install_plan(&os(MINT))),
        ["apt-get update", "apt-get install -y snapper"]
    );
    assert_eq!(steps(install_plan(&os(FEDORA))), ["dnf install -y snapper"]);
    assert_eq!(steps(install_plan(&os(NOBARA))), ["dnf install -y snapper"]);
    assert_eq!(
        steps(install_plan(&os(ARCH))),
        ["pacman -S --needed --noconfirm snapper"]
    );
    assert_eq!(
        steps(install_plan(&os(CACHYOS))),
        ["pacman -S --needed --noconfirm snapper"]
    );
    assert_eq!(
        steps(install_plan(&os(TUMBLEWEED))),
        ["zypper --non-interactive install snapper"]
    );
    assert_eq!(
        steps(install_plan(&os(LEAP))),
        ["zypper --non-interactive install snapper"]
    );
    assert_eq!(
        steps(install_plan(&os(ALPINE_EDGE))),
        ["apk add --repository https://dl-cdn.alpinelinux.org/alpine/edge/testing snapper"]
    );
}

/// The families whose install plan *runs commands as root*. Each is installed for real, in a
/// fresh container, on every push (`distros` in ci.yml, `packaging/test/doctor-install.sh`).
/// A family does not join this list because its package manager looks familiar: it joins when
/// a container proves the commands work, and the matrix in ci.yml grows with it.
const RUN_FOR_REAL_IN_CI: [Family; 5] = [
    Family::Debian,
    Family::Fedora,
    Family::Arch,
    Family::Suse,
    Family::Alpine,
];

#[test]
fn only_families_that_ci_installs_for_real_run_commands_as_root() {
    let systems = [
        DEBIAN,
        UBUNTU,
        MINT,
        FEDORA,
        SILVERBLUE,
        ROCKY,
        NOBARA,
        ARCH,
        CACHYOS,
        MANJARO,
        TUMBLEWEED,
        LEAP,
        MICROOS,
        ALPINE,
        ALPINE_EDGE,
        VOID,
        GENTOO,
        NIXOS,
        STEAMOS,
    ];
    for system in systems {
        let os = os(system);
        if let InstallPlan::Steps(_) = install_plan(&os) {
            assert!(
                RUN_FOR_REAL_IN_CI.contains(&family(&os)),
                "{} would run commands as root, but no container in CI has run them",
                os.display_name()
            );
        }
    }
}

#[test]
fn void_and_gentoo_say_what_to_run_instead_of_running_it() {
    assert!(manual(install_plan(&os(VOID))).contains("xbps-install -S snapper"));
    assert!(manual(install_plan(&os(GENTOO))).contains("emerge --ask app-backup/snapper"));
}

#[test]
fn arch_never_syncs_the_database_without_upgrading() {
    let plan = steps(install_plan(&os(ARCH))).join(" ");
    assert!(!plan.contains("-Sy") && !plan.contains("-y"), "{plan}");
}

#[test]
fn systems_where_a_command_would_be_wrong_say_so_instead_of_running_one() {
    assert!(manual(install_plan(&os(ROCKY))).contains("EPEL"));
    let stable_alpine = manual(install_plan(&os(ALPINE)));
    assert!(
        stable_alpine.contains("testing") && stable_alpine.contains("Alpine Linux v3.22"),
        "{stable_alpine}"
    );
    assert!(manual(install_plan(&os(NIXOS))).contains("environment.systemPackages"));
    assert!(manual(install_plan(&os(SILVERBLUE))).contains("rpm-ostree install snapper"));
    assert!(manual(install_plan(&os(MICROOS))).contains("transactional-update"));
    assert!(manual(install_plan(&os(STEAMOS))).contains("distrobox"));
    let unknown = manual(install_plan(&os("ID=plan9\nPRETTY_NAME=\"Plan 9\"\n")));
    assert!(
        unknown.contains("snapper.io") && unknown.contains("Plan 9"),
        "{unknown}"
    );
}

fn probe(
    snapper: Result<&str, &str>,
    root: Option<bool>,
    areas: Option<Result<&[&str], &str>>,
) -> Probe {
    Probe {
        os: os(ARCH),
        snapper: snapper.map(str::to_owned).map_err(str::to_owned),
        is_root: root,
        areas: areas.map(|r| {
            r.map(|names| names.iter().map(|n| AreaName::parse(n).unwrap()).collect())
                .map_err(str::to_owned)
        }),
        locations: Vec::new(),
    }
}

#[test]
fn a_healthy_system_has_nothing_to_fix() {
    let checks = diagnose(&probe(
        Ok("snapper 0.13.1"),
        Some(true),
        Some(Ok(&["home", "root"])),
    ));
    assert_eq!(worst(&checks), Status::Ok);
    assert!(checks.iter().all(|c| c.fix.is_none()));
    assert!(checks[2].detail.contains("2 protected: home, root"));
}

#[test]
fn a_missing_snapper_fails_and_names_the_distro_specific_fix() {
    let checks = diagnose(&probe(Err("No such file or directory"), Some(true), None));
    assert_eq!(worst(&checks), Status::Fail);
    assert_eq!(checks[0].status, Status::Fail);
    assert_eq!(
        checks[0].fix.as_deref(),
        Some("pacman -S --needed --noconfirm snapper")
    );
    assert_eq!(
        checks[2].status,
        Status::Warn,
        "areas cannot be checked, which is not a failure of their own"
    );
}

#[test]
fn no_areas_and_no_root_are_warnings_not_failures() {
    let checks = diagnose(&probe(Ok("snapper 0.13.1"), Some(false), Some(Ok(&[]))));
    assert_eq!(worst(&checks), Status::Warn);
    assert_eq!(checks[1].fix.as_deref(), Some("run with sudo"));
    assert!(checks[2].fix.as_deref().unwrap().contains("snapall setup"));
}

#[test]
fn a_failing_area_listing_is_a_failure_and_suggests_sudo_only_when_not_root() {
    let not_root = diagnose(&probe(
        Ok("snapper 0.13.1"),
        Some(false),
        Some(Err("permission denied")),
    ));
    assert_eq!(not_root[2].status, Status::Fail);
    assert_eq!(not_root[2].fix.as_deref(), Some("run with sudo"));
    let root = diagnose(&probe(
        Ok("snapper 0.13.1"),
        Some(true),
        Some(Err("failed")),
    ));
    assert_eq!(root[2].status, Status::Fail);
    assert_eq!(root[2].fix, None, "sudo would not help a root user");
}

#[test]
fn an_unreachable_dbus_is_recognised_and_points_at_no_dbus() {
    let why = "Failure (org.freedesktop.DBus.Error.FileNotFound).";
    let checks = diagnose(&probe(Ok("snapper 0.10.6"), Some(true), Some(Err(why))));
    let fix = checks[2].fix.as_deref().unwrap();
    assert!(fix.contains("--no-dbus") && fix.contains("D-Bus"), "{fix}");
}

#[test]
fn an_unknown_privilege_state_is_reported_not_guessed() {
    let checks = diagnose(&probe(Ok("snapper 0.13.1"), None, Some(Ok(&["home"]))));
    assert_eq!(checks[1].status, Status::Warn);
}

#[test]
fn describe_joins_steps_and_passes_manual_text_through() {
    assert_eq!(
        describe(&install_plan(&os(DEBIAN))),
        "apt-get update && apt-get install -y snapper"
    );
    assert_eq!(describe(&InstallPlan::Manual("do it".into())), "do it");
}
