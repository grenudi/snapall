// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What snapshots leave out. A snapper config snapshots one subvolume, and a snapshot stops at a
//! subvolume boundary, so on the usual Arch/CachyOS layout (`@` for `/`, `@home` for `/home`) the
//! `root` config does not contain a single file under `/home`. Nothing here reads the system: the
//! shell hands over the text of `/proc/self/mountinfo` and the configs' subvolumes.

/// A filesystem mounted at a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub point: String,
    pub fstype: String,
}

/// `\040` and friends: mountinfo writes a space, tab, newline or backslash in a path as an octal
/// escape.
fn unescape(field: &str) -> String {
    let mut out = String::new();
    let bytes = field.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let octal = (bytes[i] == b'\\' && i + 3 < bytes.len())
            .then(|| field.get(i + 1..i + 4))
            .flatten()
            .and_then(|digits| u8::from_str_radix(digits, 8).ok());
        match octal {
            Some(byte) => {
                out.push(char::from(byte));
                i += 4;
            }
            None => {
                out.push(char::from(bytes[i]));
                i += 1;
            }
        }
    }
    out
}

/// The mounts in the text of `/proc/self/mountinfo`. Lines that are not in the documented shape
/// (`... mount-point ... - fstype source options`) are skipped.
pub fn mounts_from_mountinfo(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let point = unescape(before.split_whitespace().nth(4)?);
            let fstype = after.split_whitespace().next()?.to_owned();
            Some(Mount { point, fstype })
        })
        .collect()
}

/// `/home` when it is a Btrfs mount of its own and no config has it as its subvolume: the one place
/// where leaving a config out silently leaves out the user's own files.
///
/// A `/home` that is only a directory of the root subvolume has no mount of its own and is covered
/// by the root config, so it is not reported.
pub fn uncovered_home(mounts: &[Mount], config_subvolumes: &[String]) -> Option<String> {
    let home = mounts
        .iter()
        .rev() // the last mount of a path is the one in effect
        .find(|mount| mount.point == "/home")?;
    let configured = config_subvolumes
        .iter()
        .any(|subvolume| subvolume.trim_end_matches('/') == "/home");
    (home.fstype == "btrfs" && !configured).then(|| home.point.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTINFO: &str = "\
24 1 0:21 /@ / rw,noatime shared:1 - btrfs /dev/nvme0n1p2 rw,ssd,subvolid=256,subvol=/@
25 24 0:21 /@home /home rw,noatime shared:1 - btrfs /dev/nvme0n1p2 rw,ssd,subvolid=257,subvol=/@home
26 24 0:21 /@log /var/log rw,noatime shared:1 - btrfs /dev/nvme0n1p2 rw,ssd,subvolid=258,subvol=/@log
27 24 259:1 / /boot rw,relatime shared:2 - vfat /dev/nvme0n1p1 rw,fmask=0022
28 24 0:30 / /mnt/my\\040data rw,relatime shared:3 - ext4 /dev/sda1 rw
";

    #[test]
    fn mounts_are_read_with_their_filesystem_and_escaped_paths() {
        let mounts = mounts_from_mountinfo(MOUNTINFO);
        let find = |point: &str| {
            mounts
                .iter()
                .find(|m| m.point == point)
                .map(|m| m.fstype.as_str())
        };
        assert_eq!(find("/"), Some("btrfs"));
        assert_eq!(find("/home"), Some("btrfs"));
        assert_eq!(find("/boot"), Some("vfat"));
        assert_eq!(find("/mnt/my data"), Some("ext4"), "\\040 is a space");
        assert!(mounts_from_mountinfo("garbage\n\nnot - enough\n").is_empty());
    }

    #[test]
    fn a_separate_btrfs_home_with_no_config_is_reported() {
        let mounts = mounts_from_mountinfo(MOUNTINFO);
        assert_eq!(uncovered_home(&mounts, &["/".into()]), Some("/home".into()));
    }

    #[test]
    fn a_config_for_home_settles_it_whatever_else_exists() {
        let mounts = mounts_from_mountinfo(MOUNTINFO);
        assert_eq!(uncovered_home(&mounts, &["/".into(), "/home".into()]), None);
        assert_eq!(
            uncovered_home(&mounts, &["/home/".into()]),
            None,
            "a trailing slash is the same directory"
        );
    }

    #[test]
    fn a_home_that_is_not_its_own_btrfs_mount_is_not_a_finding() {
        // a directory of the root subvolume: no mount of its own, covered by the root config
        let only_root = mounts_from_mountinfo("24 1 0:21 /@ / rw - btrfs /dev/x rw,subvol=/@\n");
        assert_eq!(uncovered_home(&only_root, &["/".into()]), None);
        // another filesystem altogether: not something snapper's Btrfs snapshots could cover
        let ext4 = mounts_from_mountinfo("30 24 8:3 / /home rw - ext4 /dev/sda3 rw\n");
        assert_eq!(uncovered_home(&ext4, &["/".into()]), None);
    }

    #[test]
    fn the_mount_in_effect_is_the_last_one() {
        let stacked = mounts_from_mountinfo(
            "30 24 8:3 / /home rw - ext4 /dev/sda3 rw\n31 24 0:21 /@home /home rw - btrfs /dev/x rw\n",
        );
        assert_eq!(uncovered_home(&stacked, &[]), Some("/home".into()));
    }
}
