// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What to tell someone whose file no snapshot can contain: why, and what to do about it, in
//! snapall's own words and with snapall's own command.

use snapall_core::protecting::coverage::{self, Mount};
use snapall_core::HistoryError;

/// The mount that holds `path`: the one with the longest mount point that `path` is under.
fn holder<'a>(mounts: &'a [Mount], path: &str) -> Option<&'a Mount> {
    mounts
        .iter()
        .filter(|mount| {
            let point = mount.point.trim_end_matches('/');
            point.is_empty() || path == point || path.starts_with(&format!("{point}/"))
        })
        .max_by_key(|mount| mount.point.len())
}

/// The error as a person should read it. Where it means "no snapshot can contain this file", it says
/// where the file is and what to do, given the machine's `mounts`.
pub fn explain_with(error: &HistoryError, path: &str, mounts: &[Mount]) -> String {
    if !matches!(
        error,
        HistoryError::OtherLocation { .. } | HistoryError::NoArea { .. }
    ) {
        return error.to_string();
    }
    let Some(holder) = holder(mounts, path) else {
        return format!(
            "No snapshot contains {path}, and snapall could not tell where on the disk it is."
        );
    };
    if holder.fstype != "btrfs" {
        return format!(
            "No snapshot contains {path}.\n\nIt is on {point}, a part of the disk snapall cannot protect: it only works with disks formatted as Btrfs.",
            point = holder.point
        );
    }
    let info = coverage::describe(&holder.point);
    format!(
        "No snapshot contains {path}.\n\nIt is in {title} ({point}). Snapshots are kept for one part of the disk at a time, and none of the parts they are kept for includes it.\n\nTo see whether {title} is protected, and protect it if it is not, run:\n\n    sudo snapall setup\n\nIt explains what protecting means and does it for you. Snapshots taken after that will include this file.",
        title = info.title,
        point = holder.point
    )
}

/// [`explain_with`] for this machine.
pub fn explain(error: &HistoryError, path: &str) -> String {
    let mountinfo = snapall_system::environment::mounts().unwrap_or_default();
    explain_with(error, path, &coverage::mounts_from_mountinfo(&mountinfo))
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::AreaName;

    const MOUNTINFO: &str = "\
24 1 0:21 /@ / rw - btrfs /dev/x rw,subvol=/@
25 24 0:21 /@home /home rw - btrfs /dev/x rw,subvol=/@home
30 24 8:3 / /mnt/usb rw - ext4 /dev/sdb1 rw
";

    fn not_covered(path: &str) -> HistoryError {
        HistoryError::OtherLocation {
            path: path.into(),
            area: AreaName::parse("root").unwrap(),
            location: "/".into(),
        }
    }

    fn mounts() -> Vec<Mount> {
        coverage::mounts_from_mountinfo(MOUNTINFO)
    }

    #[test]
    fn a_file_in_a_btrfs_area_is_pointed_at_setup_in_plain_words() {
        let text = explain_with(
            &not_covered("/home/me/notes.txt"),
            "/home/me/notes.txt",
            &mounts(),
        );
        for must in [
            "No snapshot contains /home/me/notes.txt",
            "It is in Home (/home)",
            "sudo snapall setup",
            "explains what protecting means",
        ] {
            assert!(text.contains(must), "`{must}` missing:\n{text}");
        }
        for jargon in ["snapper", "config", "subvolume", "create-config"] {
            assert!(
                !text.to_lowercase().contains(jargon),
                "`{jargon}` in:\n{text}"
            );
        }
    }

    #[test]
    fn the_most_specific_mount_names_the_area_not_the_one_above_it() {
        let text = explain_with(&not_covered("/home/me/a"), "/home/me/a", &mounts());
        assert!(
            text.contains("Home (/home)") && !text.contains("System (/)"),
            "{text}"
        );
        let system = explain_with(&not_covered("/etc/fstab"), "/etc/fstab", &mounts());
        assert!(system.contains("System (/)"), "{system}");
    }

    #[test]
    fn a_file_on_a_disk_snapall_cannot_protect_says_so_and_does_not_promise_setup_will_help() {
        let text = explain_with(
            &not_covered("/mnt/usb/photo.jpg"),
            "/mnt/usb/photo.jpg",
            &mounts(),
        );
        assert!(
            text.contains("cannot protect") && text.contains("Btrfs"),
            "{text}"
        );
        assert!(!text.contains("sudo snapall setup"), "{text}");
    }

    #[test]
    fn other_errors_and_unknown_places_are_not_dressed_up() {
        let bad = explain_with(
            &HistoryError::BadPath { path: "rel".into() },
            "rel",
            &mounts(),
        );
        assert!(!bad.contains("setup"), "{bad}");
        let lost = explain_with(&not_covered("/x"), "/x", &[]);
        assert!(lost.contains("could not tell where"), "{lost}");
    }
}
