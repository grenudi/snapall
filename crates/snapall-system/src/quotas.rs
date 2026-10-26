// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a disk counts the room snapshots take, and switching it on and off.

use std::process::Command;

use snapall_core::QuotaMode;

/// The mode from what `btrfs qgroup show` did and what the kernel says its mode is
/// (`/sys/fs/btrfs/<uuid>/qgroups/mode`: `qgroup` or `squota`; absent on older kernels).
pub fn mode_from(listed: bool, said: &str, kernel_mode: Option<&str>) -> QuotaMode {
    if !listed {
        return if said.contains("quotas not enabled") {
            QuotaMode::Off
        } else {
            QuotaMode::Unknown
        };
    }
    match kernel_mode.map(str::trim) {
        Some("squota") => QuotaMode::Simple,
        // listed with the full kind, or by a kernel too old to say which: the full kind is all there was
        _ => QuotaMode::Full,
    }
}

/// The uuid in the first line of `btrfs filesystem show`: `Label: none  uuid: a0919197-...`.
pub fn uuid_from(show: &str) -> Option<String> {
    let line = show.lines().next()?;
    let after = line.split("uuid:").nth(1)?.trim();
    let uuid = after.split_whitespace().next()?;
    (!uuid.is_empty()).then(|| uuid.to_owned())
}

pub fn mode(path: &str) -> QuotaMode {
    let Ok(listing) = Command::new("btrfs")
        .args(["qgroup", "show", path])
        .output()
    else {
        return QuotaMode::Unknown;
    };
    let said = String::from_utf8_lossy(&listing.stderr);
    let kernel_mode = Command::new("btrfs")
        .args(["filesystem", "show", path])
        .output()
        .ok()
        .and_then(|out| uuid_from(&String::from_utf8_lossy(&out.stdout)))
        .and_then(|uuid| {
            std::fs::read_to_string(format!("/sys/fs/btrfs/{uuid}/qgroups/mode")).ok()
        });
    mode_from(listing.status.success(), &said, kernel_mode.as_deref())
}

/// Switch counting on (and start building the tally) or off. Enabling twice is harmless.
pub fn set(path: &str, on: bool) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err(format!("{path} is not an absolute path"));
    }
    let run = |args: &[&str]| -> Result<(), String> {
        let output = Command::new("btrfs")
            .args(args)
            .output()
            .map_err(|e| format!("btrfs could not be run: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(plain(&String::from_utf8_lossy(&output.stderr)))
        }
    };
    if on {
        // enabling starts building the tally in the background by itself (a rescan of our own would be
        // refused as "already in progress"); the figures are right once it has finished
        run(&["quota", "enable", path])
    } else {
        run(&["quota", "disable", path])
    }
}

/// btrfs's complaint, with the one that matters most said in plain words.
fn plain(stderr: &str) -> String {
    let text = stderr.trim();
    if text.contains("Operation not permitted") || text.contains("Permission denied") {
        "this needs administrator rights: start snapall with sudo".to_owned()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_is_told_from_what_btrfs_said_and_what_the_kernel_says() {
        assert_eq!(
            mode_from(false, "ERROR: can't list qgroups: quotas not enabled", None),
            QuotaMode::Off
        );
        assert_eq!(
            mode_from(false, "something else broke", None),
            QuotaMode::Unknown,
            "an error is not 'off'"
        );
        assert_eq!(mode_from(true, "", Some("qgroup\n")), QuotaMode::Full);
        assert_eq!(mode_from(true, "", Some("squota\n")), QuotaMode::Simple);
        assert_eq!(
            mode_from(true, "", None),
            QuotaMode::Full,
            "an older kernel has only the full kind"
        );
    }

    #[test]
    fn the_filesystem_is_found_from_the_first_line_of_btrfs_filesystem_show() {
        assert_eq!(
            uuid_from(
                "Label: none  uuid: a0919197-eec3-4e73-b2f7-c0feadcf7525\n\tTotal devices 1\n"
            )
            .as_deref(),
            Some("a0919197-eec3-4e73-b2f7-c0feadcf7525")
        );
        assert_eq!(uuid_from(""), None);
        assert_eq!(uuid_from("no identity here"), None);
    }

    #[test]
    fn a_relative_path_is_refused_and_a_permission_error_is_said_plainly() {
        assert!(set("relative/path", true)
            .unwrap_err()
            .contains("not an absolute path"));
        assert!(plain("ERROR: Could not open: Operation not permitted")
            .contains("administrator rights"));
        assert_eq!(plain("  ERROR: other \n"), "ERROR: other");
    }
}
