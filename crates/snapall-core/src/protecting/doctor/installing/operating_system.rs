// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Understanding the operating system: what it is called, which family it belongs to, and whether it can be changed.

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
