// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Can a snapshot be booted? The question behind "restore the system".
//!
//! snapall does **not** write bootloader entries and does **not** restore the system root
//! itself (ADR 12). It reads the machine, says what is there, and points at the tool that does
//! the job properly: `limine-snapper-sync` for Limine, `grub-btrfs` for GRUB. This module is the
//! judging part: [`assess`] turns facts about the machine (gathered elsewhere) into a verdict,
//! and [`hint`] and [`restore_route`] turn a verdict into what to tell the user. Nothing here
//! reads the system.

/// A bootloader snapall knows how to recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bootloader {
    Limine,
    Grub,
    SystemdBoot,
}

impl Bootloader {
    pub fn name(self) -> &'static str {
        match self {
            Bootloader::Limine => "Limine",
            Bootloader::Grub => "GRUB",
            Bootloader::SystemdBoot => "systemd-boot",
        }
    }
}

/// A maintained tool that puts snapshots in a boot menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    LimineSnapperSync,
    GrubBtrfs,
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::LimineSnapperSync => "limine-snapper-sync",
            Tool::GrubBtrfs => "grub-btrfs",
        }
    }

    pub fn bootloader(self) -> Bootloader {
        match self {
            Tool::LimineSnapperSync => Bootloader::Limine,
            Tool::GrubBtrfs => Bootloader::Grub,
        }
    }

    fn for_bootloader(bootloader: Bootloader) -> Option<Tool> {
        match bootloader {
            Bootloader::Limine => Some(Tool::LimineSnapperSync),
            Bootloader::Grub => Some(Tool::GrubBtrfs),
            Bootloader::SystemdBoot => None,
        }
    }

    pub fn homepage(self) -> &'static str {
        match self {
            Tool::LimineSnapperSync => "https://gitlab.com/Zesko/limine-snapper-sync",
            Tool::GrubBtrfs => "https://github.com/Antynea/grub-btrfs",
        }
    }
}

/// What was found about one tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolFacts {
    pub tool: Tool,
    pub installed: bool,
    /// Running, or (for GRUB) already present in the generated menu.
    pub active: bool,
}

/// Facts about this machine, gathered by the shell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BootFacts {
    /// The filesystem type of `/`, if it could be read.
    pub root_fs: Option<String>,
    /// Every bootloader whose files were found.
    pub bootloaders: Vec<Bootloader>,
    pub tools: Vec<ToolFacts>,
}

/// The verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootStatus {
    /// `/` is not Btrfs, so there are no bootable snapshots to speak of.
    NotApplicable { root_fs: Option<String> },
    /// A tool is installed and running: snapshots are (or will be) in the boot menu.
    Active { tool: Tool },
    /// A tool is installed but not running.
    Installed { tool: Tool },
    /// The bootloader has a tool and it is not installed.
    Missing { bootloader: Bootloader, tool: Tool },
    /// The bootloader is known, and snapall knows no maintained tool for it.
    NoKnownTool { bootloader: Bootloader },
    /// Zero or several bootloaders were found, so snapall will not guess.
    UnknownBootloader { found: Vec<Bootloader> },
}

/// Judge the facts. Pure.
pub fn assess(facts: &BootFacts) -> BootStatus {
    if facts.root_fs.as_deref() != Some("btrfs") {
        return BootStatus::NotApplicable {
            root_fs: facts.root_fs.clone(),
        };
    }
    if let Some(found) = facts.tools.iter().find(|t| t.installed && t.active) {
        return BootStatus::Active { tool: found.tool };
    }
    if let Some(found) = facts.tools.iter().find(|t| t.installed) {
        return BootStatus::Installed { tool: found.tool };
    }
    match facts.bootloaders.as_slice() {
        [bootloader] => match Tool::for_bootloader(*bootloader) {
            Some(tool) => BootStatus::Missing {
                bootloader: *bootloader,
                tool,
            },
            None => BootStatus::NoKnownTool {
                bootloader: *bootloader,
            },
        },
        found => BootStatus::UnknownBootloader {
            found: found.to_vec(),
        },
    }
}

impl BootStatus {
    /// Whether a one-line nudge belongs after taking a snapshot.
    pub fn deserves_a_hint(&self) -> bool {
        matches!(
            self,
            BootStatus::Installed { .. }
                | BootStatus::Missing { .. }
                | BootStatus::NoKnownTool { .. }
        )
    }
}

/// What a boot-menu entry for a snapshot gives you, in one sentence.
pub const WHAT_IT_GIVES: &str = "a snapshot appears as an entry in the boot menu, so if an update leaves the system unable to start you can still boot an earlier state and restore it";

/// One line for after a snapshot is taken, or `None` when there is nothing to suggest.
pub fn hint(status: &BootStatus) -> Option<String> {
    match status {
        BootStatus::Missing { tool, .. } => Some(format!(
            "tip: with {} installed, {WHAT_IT_GIVES}. See `snapall doctor`.",
            tool.name()
        )),
        BootStatus::Installed { tool } => Some(format!(
            "tip: {} is installed but not running, so snapshots are not in the boot menu. See `snapall doctor`.",
            tool.name()
        )),
        BootStatus::NoKnownTool { bootloader } => Some(format!(
            "tip: snapall knows no tool that adds snapshots to the {} menu; restoring files still works (`snapall restore`).",
            bootloader.name()
        )),
        _ => None,
    }
}

/// The `snapall doctor` detail: what was found, what it gives, how to use or get it. Empty for
/// `NotApplicable` (no row at all).
pub fn details(status: &BootStatus) -> Vec<String> {
    match status {
        BootStatus::NotApplicable { .. } => Vec::new(),
        BootStatus::Active { tool } => vec![
            format!("snapshots reach the {} menu through {}", tool.bootloader().name(), tool.name()),
            restore_route(status),
        ],
        BootStatus::Installed { tool } => vec![
            format!("{} is installed but not running", tool.name()),
            match tool {
                Tool::LimineSnapperSync => "start it with: sudo systemctl enable --now limine-snapper-sync.service".to_owned(),
                Tool::GrubBtrfs => "start it with: sudo systemctl enable --now grub-btrfsd.service (or regenerate the menu with grub-mkconfig)".to_owned(),
            },
            format!("what it gives: {WHAT_IT_GIVES}"),
        ],
        BootStatus::Missing { bootloader, tool } => vec![
            format!("this system boots with {} on Btrfs, and no tool puts its snapshots in the boot menu", bootloader.name()),
            format!("{} does that: {}", tool.name(), tool.homepage()),
            format!("what it gives: {WHAT_IT_GIVES}"),
            "snapall does not edit your bootloader itself: a wrong entry can leave a system that does not start".to_owned(),
        ],
        BootStatus::NoKnownTool { bootloader } => vec![
            format!("this system boots with {} on Btrfs; snapall knows no maintained tool that adds snapshots to its menu", bootloader.name()),
            "restoring files from a snapshot still works: snapall restore".to_owned(),
        ],
        BootStatus::UnknownBootloader { found } => vec![if found.is_empty() {
            "snapall could not tell which bootloader this system uses".to_owned()
        } else {
            format!(
                "several bootloaders were found ({}); snapall will not guess which one boots",
                found.iter().map(|b| b.name()).collect::<Vec<_>>().join(", ")
            )
        }],
    }
}

/// How to restore *the system itself* (not files) from a snapshot, for the machine as found.
/// Shown when someone asks to restore the system root, which snapall refuses to do.
pub fn restore_route(status: &BootStatus) -> String {
    match status {
        BootStatus::Active { tool: Tool::LimineSnapperSync } => "to restore the system: reboot, pick the snapshot in the Limine menu, and when it has booted run `sudo limine-snapper-restore`. It restores the matching kernel too: restoring only the snapshot leaves a system whose kernel and initramfs (kept outside Btrfs) do not match, which is why snapall does not do it".to_owned(),
        BootStatus::Active { tool: Tool::GrubBtrfs } => "to restore the system: reboot, pick the snapshot from the GRUB snapshots submenu to look around, then restore it with the procedure for your subvolume layout (https://wiki.archlinux.org/title/Snapper#Restore_snapshot). grub-btrfs only adds the boot entries".to_owned(),
        _ => "to restore the system: boot from a live USB and follow https://wiki.archlinux.org/title/Snapper#Restore_snapshot. A running root cannot be replaced safely, so snapall restores single files of it, and whole non-root configs such as home".to_owned(),
    }
}

/// The filesystem type of `/`, from the text of `/proc/mounts`. The last line for `/` wins, as
/// the last mount is the one in effect.
pub fn root_fs_from_mounts(mounts: &str) -> Option<String> {
    mounts
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (_device, mountpoint, fstype) = (fields.next()?, fields.next()?, fields.next()?);
            (mountpoint == "/").then(|| fstype.to_owned())
        })
        .next_back()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(root_fs: &str, bootloaders: &[Bootloader], tools: &[ToolFacts]) -> BootFacts {
        BootFacts {
            root_fs: Some(root_fs.to_owned()),
            bootloaders: bootloaders.to_vec(),
            tools: tools.to_vec(),
        }
    }

    fn tool(tool: Tool, installed: bool, active: bool) -> ToolFacts {
        ToolFacts {
            tool,
            installed,
            active,
        }
    }

    #[test]
    fn a_root_that_is_not_btrfs_has_nothing_to_say() {
        for fs in ["ext4", "xfs", "zfs"] {
            let status = assess(&facts(fs, &[Bootloader::Grub], &[]));
            assert!(matches!(status, BootStatus::NotApplicable { .. }), "{fs}");
            assert_eq!(hint(&status), None);
            assert!(details(&status).is_empty());
        }
        assert!(matches!(
            assess(&BootFacts::default()),
            BootStatus::NotApplicable { root_fs: None }
        ));
    }

    #[test]
    fn a_running_tool_means_snapshots_are_in_the_menu_and_no_hint_is_shown() {
        let status = assess(&facts(
            "btrfs",
            &[Bootloader::Limine],
            &[tool(Tool::LimineSnapperSync, true, true)],
        ));
        assert_eq!(
            status,
            BootStatus::Active {
                tool: Tool::LimineSnapperSync
            }
        );
        assert_eq!(
            hint(&status),
            None,
            "nothing to suggest: it is already there"
        );
        assert!(details(&status)[1].contains("limine-snapper-restore"));
    }

    #[test]
    fn an_installed_tool_that_is_not_running_says_how_to_start_it() {
        let status = assess(&facts(
            "btrfs",
            &[Bootloader::Limine],
            &[tool(Tool::LimineSnapperSync, true, false)],
        ));
        assert_eq!(
            status,
            BootStatus::Installed {
                tool: Tool::LimineSnapperSync
            }
        );
        assert!(status.deserves_a_hint());
        assert!(details(&status)
            .iter()
            .any(|l| l.contains("enable --now limine-snapper-sync")));
    }

    #[test]
    fn a_known_bootloader_without_its_tool_names_the_tool_and_what_it_gives() {
        for (loader, expected) in [
            (Bootloader::Limine, Tool::LimineSnapperSync),
            (Bootloader::Grub, Tool::GrubBtrfs),
        ] {
            let status = assess(&facts("btrfs", &[loader], &[]));
            assert_eq!(
                status,
                BootStatus::Missing {
                    bootloader: loader,
                    tool: expected
                }
            );
            let hint = hint(&status).unwrap();
            assert!(
                hint.contains(expected.name()) && hint.contains("boot menu"),
                "{hint}"
            );
            let detail = details(&status).join("\n");
            assert!(detail.contains(expected.homepage()), "{detail}");
            assert!(detail.contains("does not edit your bootloader"), "{detail}");
        }
    }

    #[test]
    fn systemd_boot_is_recognised_and_snapall_admits_it_knows_no_tool() {
        let status = assess(&facts("btrfs", &[Bootloader::SystemdBoot], &[]));
        assert_eq!(
            status,
            BootStatus::NoKnownTool {
                bootloader: Bootloader::SystemdBoot
            }
        );
        assert!(hint(&status).unwrap().contains("knows no tool"));
    }

    #[test]
    fn zero_or_several_bootloaders_are_not_guessed_at() {
        let none = assess(&facts("btrfs", &[], &[]));
        assert!(matches!(&none, BootStatus::UnknownBootloader { found } if found.is_empty()));
        let two = assess(&facts(
            "btrfs",
            &[Bootloader::Limine, Bootloader::Grub],
            &[],
        ));
        assert!(matches!(&two, BootStatus::UnknownBootloader { found } if found.len() == 2));
        assert!(details(&two)[0].contains("will not guess"));
        assert_eq!(hint(&two), None, "no nagging when snapall does not know");
    }

    #[test]
    fn the_restore_route_is_the_supported_one_for_the_machine_and_never_a_blind_restore() {
        let limine = restore_route(&BootStatus::Active {
            tool: Tool::LimineSnapperSync,
        });
        assert!(limine.contains("limine-snapper-restore") && limine.contains("kernel"));
        let grub = restore_route(&BootStatus::Active {
            tool: Tool::GrubBtrfs,
        });
        assert!(grub.contains("wiki.archlinux.org"));
        let other = restore_route(&BootStatus::UnknownBootloader { found: vec![] });
        assert!(other.contains("live USB"));
    }

    #[test]
    fn the_root_filesystem_is_read_from_the_last_mount_of_slash() {
        let mounts = "sysfs /sys sysfs rw 0 0\n/dev/sda2 / ext4 rw 0 0\n/dev/nvme0n1p2 / btrfs rw,subvol=/@ 0 0\n/dev/sda1 /boot vfat rw 0 0\n";
        assert_eq!(root_fs_from_mounts(mounts).as_deref(), Some("btrfs"));
        assert_eq!(root_fs_from_mounts("proc /proc proc rw 0 0\n"), None);
        assert_eq!(root_fs_from_mounts(""), None);
    }
}
