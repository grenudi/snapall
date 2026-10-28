// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What this machine is, for the doctor and for setup: its operating system, whether this is the administrator,
//! and its mount table.

use snapall_core::protecting::doctor::OsRelease;

pub fn operating_system() -> OsRelease {
    ["/etc/os-release", "/usr/lib/os-release"]
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| OsRelease::parse(&text))
        .unwrap_or_default()
}

/// Whether the effective user is root, if that can be told.
pub fn is_administrator() -> Option<bool> {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| effective_uid(&status))
        .map(|uid| uid == 0)
}

/// The mount table as the kernel prints it, if it can be read.
pub fn mounts() -> Option<String> {
    std::fs::read_to_string("/proc/self/mountinfo").ok()
}

/// The effective uid from the text of `/proc/self/status` (`Uid: real effective saved fs`).
fn effective_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|rest| rest.split_whitespace().nth(1))
        .and_then(|uid| uid.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_effective_uid_is_the_second_field() {
        let status =
            "Name:\tsnapall\nUmask:\t0022\nUid:\t1000\t0\t0\t0\nGid:\t1000\t1000\t1000\t1000\n";
        assert_eq!(effective_uid(status), Some(0));
        assert_eq!(effective_uid("Uid:\t0\t1000\t0\t0\n"), Some(1000));
    }

    #[test]
    fn an_unreadable_status_is_unknown_not_root() {
        assert_eq!(effective_uid(""), None);
        assert_eq!(effective_uid("Uid:\n"), None);
        assert_eq!(effective_uid("Uid:\tx\ty\n"), None);
    }
}
