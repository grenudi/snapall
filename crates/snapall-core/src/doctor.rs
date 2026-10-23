//! `snapall doctor`, as pure functions: from facts about the machine to findings and an
//! install plan. Nothing here reads a file, runs a program or asks who the user is; the
//! shell probes the machine and hands the facts in, so every distribution is a unit test.

use crate::domain::ConfigName;

/// The package that provides everything snapall drives.
pub const PACKAGE: &str = "snapper";

// ------------------------------------------------------------------ os-release

/// The fields of `/etc/os-release` that decide how to install a package.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OsRelease {
    pub id: String,
    pub id_like: Vec<String>,
    pub variant_id: Option<String>,
    pub pretty_name: Option<String>,
}

impl OsRelease {
    /// Parse the `KEY=value` / `KEY="value"` format. Total: unknown or empty input yields an
    /// empty `OsRelease`, which classifies as [`Family::Unknown`].
    pub fn parse(text: &str) -> Self {
        let mut os = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            match key.trim() {
                "ID" => os.id = value.to_ascii_lowercase(),
                "ID_LIKE" => {
                    os.id_like = value
                        .split_whitespace()
                        .map(str::to_ascii_lowercase)
                        .collect();
                }
                "VARIANT_ID" => os.variant_id = Some(value.to_ascii_lowercase()),
                "PRETTY_NAME" => os.pretty_name = Some(value.to_owned()),
                _ => {}
            }
        }
        os
    }

    /// A name to show a human.
    pub fn display_name(&self) -> &str {
        self.pretty_name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(if self.id.is_empty() {
                "an unknown system"
            } else {
                &self.id
            })
    }
}

/// How a distribution installs software.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Debian,
    Fedora,
    /// RHEL and its rebuilds: snapper is only in EPEL there.
    Rhel,
    Arch,
    Suse,
    Alpine,
    Void,
    Gentoo,
    Nix,
    Unknown,
}

fn family_of_token(token: &str) -> Option<Family> {
    Some(match token {
        "debian" | "ubuntu" => Family::Debian,
        "fedora" => Family::Fedora,
        "rhel" | "centos" | "rocky" | "almalinux" | "ol" | "amzn" | "scientific" => Family::Rhel,
        "arch" => Family::Arch,
        "suse" | "sles" | "sled" => Family::Suse,
        t if t.starts_with("opensuse") => Family::Suse,
        "alpine" => Family::Alpine,
        "void" => Family::Void,
        "gentoo" => Family::Gentoo,
        "nixos" => Family::Nix,
        _ => return None,
    })
}

/// Classify a system by its own ID first, then by the distributions it says it derives from.
pub fn family(os: &OsRelease) -> Family {
    if let Some(family) = family_of_token(&os.id) {
        return family;
    }
    // A derivative that lists both `rhel` and `fedora` (Nobara, for one) installs like Fedora.
    if os.id_like.iter().any(|t| t == "fedora") {
        return Family::Fedora;
    }
    os.id_like
        .iter()
        .find_map(|token| family_of_token(token))
        .unwrap_or(Family::Unknown)
}

/// Systems where the root filesystem is read-only and packages are layered another way.
fn immutable(os: &OsRelease) -> Option<&'static str> {
    const FEDORA_ATOMIC: &[&str] = &[
        "silverblue",
        "kinoite",
        "sericea",
        "onyx",
        "iot",
        "coreos",
        "cosmic-atomic",
    ];
    if os.id == "fedora"
        && os
            .variant_id
            .as_deref()
            .is_some_and(|v| FEDORA_ATOMIC.contains(&v))
    {
        return Some("rpm-ostree install snapper   (then reboot)");
    }
    match os.id.as_str() {
        "opensuse-microos" | "opensuse-aeon" | "opensuse-kalpa" => {
            Some("transactional-update pkg install snapper   (then reboot)")
        }
        "steamos" | "bazzite" | "chimeraos" => Some(
            "install snapper in a container such as distrobox, or through your system's own layering tool",
        ),
        _ => None,
    }
}

// --------------------------------------------------------------- install plan

/// One program invocation. No shell is ever involved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub program: &'static str,
    pub args: Vec<&'static str>,
}

impl Step {
    fn new(program: &'static str, args: &[&'static str]) -> Self {
        Self {
            program,
            args: args.to_vec(),
        }
    }

    /// The step as one line a human can read and copy.
    pub fn display(&self) -> String {
        std::iter::once(self.program)
            .chain(self.args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// What to do about a missing package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallPlan {
    /// snapall can run these, in order, after the user agrees.
    Steps(Vec<Step>),
    /// snapall will not run anything: the right action is not one command it can be sure of.
    Manual(String),
}

/// Alpine `edge` calls itself that: `PRETTY_NAME="Alpine Linux edge"`.
fn alpine_edge(os: &OsRelease) -> bool {
    os.pretty_name
        .as_deref()
        .is_some_and(|name| name.to_ascii_lowercase().contains("edge"))
}

/// How to install [`PACKAGE`] on this system.
pub fn install_plan(os: &OsRelease) -> InstallPlan {
    if let Some(guidance) = immutable(os) {
        return InstallPlan::Manual(format!(
            "{} has an immutable root; do this yourself: {guidance}",
            os.display_name()
        ));
    }
    match family(os) {
        Family::Debian => InstallPlan::Steps(vec![
            Step::new("apt-get", &["update"]),
            Step::new("apt-get", &["install", "-y", PACKAGE]),
        ]),
        Family::Fedora => InstallPlan::Steps(vec![Step::new("dnf", &["install", "-y", PACKAGE])]),
        Family::Rhel => InstallPlan::Manual(
            "snapper is not in the base repositories of RHEL and its rebuilds, only in EPEL: \
             enable EPEL (and CRB / PowerTools), then run `dnf install snapper`"
                .to_owned(),
        ),
        // No `-y`/`-Sy`: syncing the database without upgrading is how partial upgrades happen.
        Family::Arch => InstallPlan::Steps(vec![Step::new(
            "pacman",
            &["-S", "--needed", "--noconfirm", PACKAGE],
        )]),
        Family::Suse => InstallPlan::Steps(vec![Step::new(
            "zypper",
            &["--non-interactive", "install", PACKAGE],
        )]),
        // snapper is only in Alpine's `testing` repository, which exists on `edge` and nowhere
        // else; mixing it into a stable release would break that release's dependencies.
        Family::Alpine if alpine_edge(os) => InstallPlan::Steps(vec![Step::new(
            "apk",
            &[
                "add",
                "--repository",
                "https://dl-cdn.alpinelinux.org/alpine/edge/testing",
                PACKAGE,
            ],
        )]),
        Family::Alpine => InstallPlan::Manual(format!(
            "snapper is only packaged in Alpine's `testing` repository, which exists on `edge`; \
             {} is not edge, so there is no snapper for it (and mixing edge into a stable \
             release breaks it)",
            os.display_name()
        )),
        Family::Void => InstallPlan::Steps(vec![Step::new("xbps-install", &["-Sy", PACKAGE])]),
        Family::Gentoo => InstallPlan::Steps(vec![Step::new(
            "emerge",
            &["--noreplace", "app-backup/snapper"],
        )]),
        Family::Nix => InstallPlan::Manual(
            "NixOS is declarative: add `snapper` to environment.systemPackages (or enable \
             services.snapper) and rebuild"
                .to_owned(),
        ),
        Family::Unknown => InstallPlan::Manual(format!(
            "install the `{PACKAGE}` package with your distribution's package manager \
             (see https://snapper.io); snapall does not recognise {}",
            os.display_name()
        )),
    }
}

/// An install plan as text, for a hint next to a failed check.
pub fn describe(plan: &InstallPlan) -> String {
    match plan {
        InstallPlan::Steps(steps) => steps
            .iter()
            .map(Step::display)
            .collect::<Vec<_>>()
            .join(" && "),
        InstallPlan::Manual(text) => text.clone(),
    }
}

// ------------------------------------------------------------------ diagnosis

/// Facts about the machine, gathered by the shell.
#[derive(Debug, Clone)]
pub struct Probe {
    pub os: OsRelease,
    /// `snapper --version`'s first line, or why it could not be run.
    pub snapper: Result<String, String>,
    /// Whether the effective user is root, if that could be determined.
    pub is_root: Option<bool>,
    /// The configs snapper reports, or why asking failed. `None` when snapper is missing.
    pub configs: Option<Result<Vec<ConfigName>, String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    /// What to do about it, when there is something to do.
    pub fix: Option<String>,
}

fn check(
    name: &'static str,
    status: Status,
    detail: impl Into<String>,
    fix: Option<String>,
) -> Check {
    Check {
        name,
        status,
        detail: detail.into(),
        fix,
    }
}

/// Turn the facts into findings, in the order a person would fix them.
pub fn diagnose(probe: &Probe) -> Vec<Check> {
    let mut checks = Vec::new();

    checks.push(match &probe.snapper {
        Ok(version) => check("snapper", Status::Ok, version.clone(), None),
        Err(why) => check(
            "snapper",
            Status::Fail,
            format!("not usable: {why}"),
            Some(describe(&install_plan(&probe.os))),
        ),
    });

    checks.push(match probe.is_root {
        Some(true) => check("privileges", Status::Ok, "running as root", None),
        Some(false) => check(
            "privileges",
            Status::Warn,
            "not root; snapper normally needs root (or permission granted in its config)",
            Some("run with sudo".to_owned()),
        ),
        None => check(
            "privileges",
            Status::Warn,
            "could not tell whether this is root",
            None,
        ),
    });

    match &probe.configs {
        None => checks.push(check(
            "configs",
            Status::Warn,
            "not checked: snapper is missing",
            None,
        )),
        Some(Ok(configs)) if configs.is_empty() => checks.push(check(
            "configs",
            Status::Warn,
            "snapper has no configs, so there is nothing to snapshot yet",
            Some("snapper -c home create-config /home".to_owned()),
        )),
        Some(Ok(configs)) => {
            let names: Vec<&str> = configs.iter().map(ConfigName::as_str).collect();
            checks.push(check(
                "configs",
                Status::Ok,
                format!("{} found: {}", names.len(), names.join(", ")),
                None,
            ));
        }
        Some(Err(why)) => checks.push(check(
            "configs",
            Status::Fail,
            format!("could not list configs: {why}"),
            config_fix(why, probe.is_root),
        )),
    }
    checks
}

/// What to try when asking snapper for its configs failed.
fn config_fix(why: &str, is_root: Option<bool>) -> Option<String> {
    if why.to_ascii_lowercase().contains("dbus") {
        // snapper talks to snapperd over the system bus. Containers and minimal installs
        // have none; as root, libsnapper can be used directly.
        return Some(
            "the system D-Bus (snapperd) is not reachable; as root, retry with --no-dbus"
                .to_owned(),
        );
    }
    (is_root == Some(false)).then(|| "run with sudo".to_owned())
}

/// The worst status among the checks (`Ok` when there are none).
pub fn worst(checks: &[Check]) -> Status {
    checks.iter().map(|c| c.status).max().unwrap_or(Status::Ok)
}

#[cfg(test)]
mod tests {
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
    const MICROOS: &str =
        "ID=\"opensuse-microos\"\nID_LIKE=\"suse opensuse opensuse-tumbleweed\"\n";
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
        assert_eq!(steps(install_plan(&os(VOID))), ["xbps-install -Sy snapper"]);
        assert_eq!(
            steps(install_plan(&os(GENTOO))),
            ["emerge --noreplace app-backup/snapper"]
        );
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
        configs: Option<Result<&[&str], &str>>,
    ) -> Probe {
        Probe {
            os: os(ARCH),
            snapper: snapper.map(str::to_owned).map_err(str::to_owned),
            is_root: root,
            configs: configs.map(|r| {
                r.map(|names| {
                    names
                        .iter()
                        .map(|n| ConfigName::parse(n).unwrap())
                        .collect()
                })
                .map_err(str::to_owned)
            }),
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
        assert!(checks[2].detail.contains("2 found: home, root"));
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
            "configs cannot be checked, which is not a failure of their own"
        );
    }

    #[test]
    fn no_configs_and_no_root_are_warnings_not_failures() {
        let checks = diagnose(&probe(Ok("snapper 0.13.1"), Some(false), Some(Ok(&[]))));
        assert_eq!(worst(&checks), Status::Warn);
        assert_eq!(checks[1].fix.as_deref(), Some("run with sudo"));
        assert!(checks[2].fix.as_deref().unwrap().contains("create-config"));
    }

    #[test]
    fn a_failing_config_listing_is_a_failure_and_suggests_sudo_only_when_not_root() {
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
}
