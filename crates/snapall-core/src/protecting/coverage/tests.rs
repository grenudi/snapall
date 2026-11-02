// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a disk has that snapshots can protect.

#![cfg(test)]

use crate::vocabulary::domain::AreaName;

use super::*;

fn name(s: &str) -> AreaName {
    AreaName::parse(s).unwrap()
}

const MOUNTINFO: &str = "\
24 1 0:21 /@ / rw,noatime shared:1 - btrfs /dev/nvme0n1p2 rw,ssd,subvolid=256,subvol=/@
25 24 0:21 /@home /home rw,noatime shared:1 - btrfs /dev/nvme0n1p2 rw,ssd,subvolid=257,subvol=/@home
26 24 0:21 /@log /var/log rw,noatime shared:1 - btrfs /dev/nvme0n1p2 rw,ssd,subvolid=258,subvol=/@log
29 25 0:21 /@home/.snapshots /home/.snapshots rw shared:1 - btrfs /dev/nvme0n1p2 rw,subvol=/@home/.snapshots
30 24 0:21 /@home /srv/bind-of-home rw shared:1 - btrfs /dev/nvme0n1p2 rw,subvol=/@home
27 24 259:1 / /boot rw,relatime shared:2 - vfat /dev/nvme0n1p1 rw,fmask=0022
28 24 0:30 / /mnt/my\\040data rw,relatime shared:3 - ext4 /dev/sda1 rw
";

fn protected(pairs: &[(&str, &str)]) -> Vec<(AreaName, String)> {
    pairs
        .iter()
        .map(|(n, s)| (name(n), (*s).to_owned()))
        .collect()
}

#[test]
fn mounts_are_read_with_filesystem_source_location_and_escaped_paths() {
    let mounts = mounts_from_mountinfo(MOUNTINFO);
    let home = mounts.iter().find(|m| m.point == "/home").unwrap();
    assert_eq!(
        (
            home.fstype.as_str(),
            home.source.as_str(),
            home.root.as_str()
        ),
        ("btrfs", "/dev/nvme0n1p2", "/@home")
    );
    assert!(
        mounts
            .iter()
            .any(|m| m.point == "/mnt/my data" && m.fstype == "ext4"),
        "\\040 is a space"
    );
    assert!(mounts_from_mountinfo("garbage\n\nnot - enough\n").is_empty());
}

#[test]
fn only_btrfs_areas_are_listed_root_first_without_snapshot_directories_or_repeats() {
    let found = areas(&mounts_from_mountinfo(MOUNTINFO), &[]);
    let points: Vec<&str> = found.iter().map(|a| a.mount_point.as_str()).collect();
    assert_eq!(points, ["/", "/home", "/var/log"], "ext4 and vfat are not Btrfs, /home/.snapshots is snapper's, and the bind mount of @home is the same area");
}

#[test]
fn an_area_is_protected_when_a_area_covers_exactly_it() {
    let mounts = mounts_from_mountinfo(MOUNTINFO);
    let none = areas(&mounts, &protected(&[("root", "/")]));
    assert_eq!(
        none.iter().map(Area::is_protected).collect::<Vec<_>>(),
        [true, false, false]
    );
    let both = areas(&mounts, &protected(&[("root", "/"), ("home", "/home/")]));
    assert_eq!(
        both[1].protected_by,
        Some(name("home")),
        "a trailing slash is the same directory"
    );
    // an area for some other directory does not protect /home
    let wrong = areas(&mounts, &protected(&[("docs", "/home/me/docs")]));
    assert!(!wrong[1].is_protected());
}

#[test]
fn every_area_is_described_in_plain_words_with_a_verdict() {
    let system = describe("/");
    assert_eq!(
        (system.title.as_str(), system.worth),
        ("System", Worth::Recommended)
    );
    let home = describe("/home/");
    assert_eq!(
        (home.title.as_str(), home.worth),
        ("Home", Worth::Recommended)
    );
    assert!(home.reason.contains("could not be undone"));
    assert_eq!(describe("/var/log").worth, Worth::NotUseful);
    assert_eq!(describe("/var/cache").worth, Worth::NotUseful);
    let other = describe("/mnt/games");
    assert_eq!(
        (other.title.as_str(), other.worth),
        ("Games", Worth::Optional)
    );
    // nobody should have to know these terms to read what we say
    for path in ["/", "/home", "/root", "/srv", "/var/log", "/tmp", "/mnt/x"] {
        let info = describe(path);
        let text = format!("{} {} {}", info.title, info.holds, info.reason).to_lowercase();
        for jargon in ["subvolume", "config", "btrfs", "snapper"] {
            assert!(
                !text.contains(jargon),
                "`{jargon}` in the words for {path}: {text}"
            );
        }
    }
}

#[test]
fn a_name_is_made_from_the_directory_is_always_valid_and_never_a_repeat() {
    let none: Vec<AreaName> = Vec::new();
    for (path, expected) in [
        ("/", "root"),
        ("/home", "home"),
        ("/home/", "home"),
        ("/srv/my data", "my_data"),
        ("/mnt/games-2", "games-2"),
        ("/mnt/\u{e4}\u{f6}", "data"),
    ] {
        assert_eq!(
            suggest_name(path, &none).unwrap().as_str(),
            expected,
            "{path}"
        );
    }
    let taken = vec![name("home"), name("home_2")];
    assert_eq!(suggest_name("/home", &taken).unwrap().as_str(), "home_3");
}
