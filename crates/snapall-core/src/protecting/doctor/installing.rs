// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Understanding the operating system, and how to install the snapshot tool on it.

pub const PACKAGE: &str = "snapper";

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

pub(super) fn family_of_token(token: &str) -> Option<Family> {
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
pub(super) fn immutable(os: &OsRelease) -> Option<&'static str> {
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
pub(super) fn alpine_edge(os: &OsRelease) -> bool {
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
        // Void and Gentoo are not installed for the user. snapall runs package-manager commands
        // as root only where it has run them for real (`packaging/test/doctor-install.sh` does,
        // on every push), and these two it has not. On Gentoo `emerge` may also compile from
        // source against a tree that was never synced, which is not a surprise to spring on
        // someone who asked to "install what is missing".
        Family::Void => InstallPlan::Manual(
            "install it with `xbps-install -S snapper`, then run `snapall doctor` again".to_owned(),
        ),
        Family::Gentoo => InstallPlan::Manual(
            "install it with `emerge --ask app-backup/snapper`, then run `snapall doctor` again"
                .to_owned(),
        ),
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
