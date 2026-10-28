// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The imperative half of the boot-menu check: look at files and services, hand the facts to
//! `snapall_core::going_back::boot::assess`. Read-only, and every path is under a root that tests replace
//! with a temporary directory.
//!
//! These are heuristics about where distributions put things; they are written from the tools'
//! own documentation, not from a run on each distribution, so a miss is reported as "could not
//! tell" and never acted upon.

use std::path::Path;
use std::process::{Command, Stdio};

use snapall_core::going_back::boot::{root_fs_from_mounts, BootFacts, Bootloader, Tool, ToolFacts};

/// Where Limine's configuration (or its helper tools' configuration) may be found.
const LIMINE: &[&str] = &[
    "boot/limine.conf",
    "boot/limine/limine.conf",
    "boot/EFI/limine/limine.conf",
    "boot/EFI/BOOT/limine.conf",
    "efi/limine.conf",
    "efi/EFI/limine/limine.conf",
    "etc/default/limine",
    "usr/bin/limine-update",
];
const GRUB: &[&str] = &[
    "boot/grub/grub.cfg",
    "boot/grub2/grub.cfg",
    "etc/default/grub",
];
const SYSTEMD_BOOT: &[&str] = &[
    "boot/loader/loader.conf",
    "efi/loader/loader.conf",
    "boot/efi/loader/loader.conf",
];

/// Gather the facts under `root` (the real `/` in production).
pub fn gather(root: &Path, service_state: &dyn Fn(&str) -> Option<String>) -> BootFacts {
    let exists = |relative: &str| root.join(relative).exists();
    let any = |paths: &[&str]| paths.iter().any(|p| exists(p));

    let mut bootloaders = Vec::new();
    if any(LIMINE) {
        bootloaders.push(Bootloader::Limine);
    }
    if any(GRUB) {
        bootloaders.push(Bootloader::Grub);
    }
    if any(SYSTEMD_BOOT) {
        bootloaders.push(Bootloader::SystemdBoot);
    }

    let limine_sync = exists("usr/bin/limine-snapper-sync");
    // The package installs a snapper plugin that runs the sync on every create, delete and modify
    // of a root snapshot, so the menu is kept in step even with the daemon stopped.
    let limine_plugin = exists("usr/lib/snapper/plugins/10-limine-snapper-sync");
    let grub_btrfs = exists("usr/bin/grub-btrfsd") || exists("etc/grub.d/41_snapshots-btrfs");
    // grub-btrfs works from the generated menu even without its daemon: the script leaves a
    // marker in grub.cfg when snapshot entries were generated.
    let grub_menu_has_snapshots =
        ["boot/grub/grub.cfg", "boot/grub2/grub.cfg"]
            .iter()
            .any(|path| {
                std::fs::read_to_string(root.join(path))
                    .is_ok_and(|text| text.contains("41_snapshots-btrfs"))
            });

    BootFacts {
        root_fs: std::fs::read_to_string(root.join("proc/mounts"))
            .ok()
            .and_then(|mounts| root_fs_from_mounts(&mounts)),
        bootloaders,
        tools: vec![
            ToolFacts {
                tool: Tool::LimineSnapperSync,
                installed: limine_sync,
                service: limine_sync
                    .then(|| service_state("limine-snapper-sync"))
                    .flatten(),
                hooked: limine_sync && limine_plugin,
            },
            ToolFacts {
                tool: Tool::GrubBtrfs,
                installed: grub_btrfs,
                service: grub_btrfs.then(|| service_state("grub-btrfsd")).flatten(),
                hooked: grub_btrfs && grub_menu_has_snapshots,
            },
        ],
    }
}

/// What `systemctl is-active` says about a unit (`active`, `inactive`, `failed`, `activating`...).
/// It prints the state whatever it exits with; no systemd or no answer is `None`.
fn systemctl_state(unit: &str) -> Option<String> {
    let output = Command::new("systemctl")
        .args(["is-active", unit])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!state.is_empty()).then_some(state)
}

/// The facts of the machine snapall is running on.
pub fn system() -> BootFacts {
    gather(Path::new("/"), &systemctl_state)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use snapall_core::going_back::boot::{assess, BootStatus};

    use super::*;

    /// A fake machine: a directory with the files a real one would have.
    struct Machine(PathBuf);

    impl Machine {
        fn new(label: &str, mounts: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("snapall-boot-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("proc")).unwrap();
            fs::write(root.join("proc/mounts"), mounts).unwrap();
            Machine(root)
        }

        fn with(self, files: &[&str]) -> Self {
            for file in files {
                let path = self.0.join(file);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, "").unwrap();
            }
            self
        }

        fn write(&self, file: &str, text: &str) {
            let path = self.0.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    impl Drop for Machine {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const BTRFS: &str = "/dev/nvme0n1p2 / btrfs rw,subvol=/@ 0 0\n";

    /// The machine's verdict, with the named units active and any other unit inactive.
    fn status(machine: &Machine, running: &[&str]) -> BootStatus {
        assess(&gather(&machine.0, &|unit| {
            Some(
                if running.contains(&unit) {
                    "active"
                } else {
                    "inactive"
                }
                .to_owned(),
            )
        }))
    }

    #[test]
    fn limine_with_its_sync_tool_running_is_active() {
        let machine = Machine::new("limine-active", BTRFS)
            .with(&["boot/limine/limine.conf", "usr/bin/limine-snapper-sync"]);
        assert_eq!(
            status(&machine, &["limine-snapper-sync"]),
            BootStatus::Active {
                tool: Tool::LimineSnapperSync
            }
        );
    }

    #[test]
    fn limine_with_its_tool_installed_but_stopped_is_installed_not_active() {
        let machine = Machine::new("limine-stopped", BTRFS)
            .with(&["etc/default/limine", "usr/bin/limine-snapper-sync"]);
        assert_eq!(
            status(&machine, &[]),
            BootStatus::Installed {
                tool: Tool::LimineSnapperSync,
                service: Some("inactive".into())
            }
        );
    }

    #[test]
    fn the_snapper_plugin_makes_limine_work_with_the_daemon_stopped() {
        let machine = Machine::new("limine-plugin", BTRFS).with(&[
            "etc/default/limine",
            "usr/bin/limine-snapper-sync",
            "usr/lib/snapper/plugins/10-limine-snapper-sync",
        ]);
        assert_eq!(
            status(&machine, &[]),
            BootStatus::Active {
                tool: Tool::LimineSnapperSync
            }
        );
    }

    #[test]
    fn the_daemons_real_state_is_carried_into_the_explanation() {
        let machine = Machine::new("limine-failed", BTRFS)
            .with(&["etc/default/limine", "usr/bin/limine-snapper-sync"]);
        let facts = gather(&machine.0, &|_| Some("failed".to_owned()));
        assert_eq!(
            assess(&facts),
            BootStatus::Installed {
                tool: Tool::LimineSnapperSync,
                service: Some("failed".into())
            }
        );
        // no systemd, no answer: the explanation says so rather than guessing
        let facts = gather(&machine.0, &|_| None);
        assert_eq!(
            assess(&facts),
            BootStatus::Installed {
                tool: Tool::LimineSnapperSync,
                service: None
            }
        );
    }

    #[test]
    fn limine_without_the_tool_is_missing_it() {
        let machine = Machine::new("limine-missing", BTRFS).with(&["boot/EFI/limine/limine.conf"]);
        assert!(matches!(
            status(&machine, &[]),
            BootStatus::Missing {
                tool: Tool::LimineSnapperSync,
                ..
            }
        ));
    }

    #[test]
    fn grub_btrfs_counts_as_active_when_its_marker_is_in_the_generated_menu() {
        let machine = Machine::new("grub-marker", BTRFS).with(&["etc/grub.d/41_snapshots-btrfs"]);
        machine.write(
            "boot/grub/grub.cfg",
            "### BEGIN /etc/grub.d/41_snapshots-btrfs ###\n",
        );
        assert_eq!(
            status(&machine, &[]),
            BootStatus::Active {
                tool: Tool::GrubBtrfs
            }
        );
    }

    #[test]
    fn grub_btrfs_installed_without_a_marker_or_daemon_is_only_installed() {
        let machine = Machine::new("grub-installed", BTRFS)
            .with(&["usr/bin/grub-btrfsd", "boot/grub/grub.cfg"]);
        assert_eq!(
            status(&machine, &[]),
            BootStatus::Installed {
                tool: Tool::GrubBtrfs,
                service: Some("inactive".into())
            }
        );
        assert_eq!(
            status(&machine, &["grub-btrfsd"]),
            BootStatus::Active {
                tool: Tool::GrubBtrfs
            }
        );
    }

    #[test]
    fn grub_and_systemd_boot_are_recognised_on_their_own() {
        let grub = Machine::new("grub", BTRFS).with(&["boot/grub/grub.cfg"]);
        assert!(matches!(
            status(&grub, &[]),
            BootStatus::Missing {
                tool: Tool::GrubBtrfs,
                ..
            }
        ));
        let sdboot = Machine::new("sdboot", BTRFS).with(&["boot/loader/loader.conf"]);
        assert!(matches!(
            status(&sdboot, &[]),
            BootStatus::NoKnownTool { .. }
        ));
    }

    #[test]
    fn two_bootloaders_or_none_are_reported_as_unknown_not_guessed() {
        let two =
            Machine::new("two", BTRFS).with(&["boot/grub/grub.cfg", "boot/limine/limine.conf"]);
        assert!(
            matches!(status(&two, &[]), BootStatus::UnknownBootloader { found } if found.len() == 2)
        );
        let none = Machine::new("none", BTRFS);
        assert!(
            matches!(status(&none, &[]), BootStatus::UnknownBootloader { found } if found.is_empty())
        );
    }

    #[test]
    fn a_machine_that_is_not_btrfs_gets_no_advice_whatever_else_is_on_it() {
        let ext4 = Machine::new("ext4", "/dev/sda2 / ext4 rw 0 0\n").with(&["boot/grub/grub.cfg"]);
        assert!(matches!(
            status(&ext4, &[]),
            BootStatus::NotApplicable { .. }
        ));
    }

    #[test]
    fn a_tool_that_is_not_installed_is_never_reported_active_even_if_a_service_of_that_name_runs() {
        let machine = Machine::new("ghost", BTRFS).with(&["boot/limine/limine.conf"]);
        assert!(matches!(
            status(&machine, &["limine-snapper-sync"]),
            BootStatus::Missing { .. }
        ));
    }
}
