// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The parts of a disk that snapshots can protect, and which of them are.
//!
//! A snapshot covers one *subvolume* (snapper calls the thing that sets one up a "config"), and it
//! stops at the boundary of that subvolume. On the usual Arch/CachyOS layout `/` and `/home` are
//! separate subvolumes, so snapshots of `/` contain none of the files under `/home`: that part of the
//! disk needs protecting in its own right. People read this module's words, so they are plain ones:
//! a *protected area* is a part of the disk snapshots are kept for.
//!
//! Nothing here reads the system: the shell hands over the text of `/proc/self/mountinfo` and the
//! subvolume of each config.

use crate::domain::AreaName;

/// A filesystem mounted at a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub point: String,
    pub fstype: String,
    /// The device (or other source) it is mounted from.
    pub source: String,
    /// Which part of that filesystem is mounted: `/@home` for the subvolume `@home`.
    pub root: String,
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
/// (`id parent dev root mount-point options ... - fstype source options`) are skipped.
pub fn mounts_from_mountinfo(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let mut head = before.split_whitespace().skip(3);
            let root = unescape(head.next()?);
            let point = unescape(head.next()?);
            let mut tail = after.split_whitespace();
            let fstype = tail.next()?.to_owned();
            let source = tail.next().unwrap_or_default().to_owned();
            Some(Mount {
                point,
                fstype,
                source,
                root,
            })
        })
        .collect()
}

/// A part of the disk that snapshots can be kept for, and whether they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    /// Where it is mounted: `/`, `/home`.
    pub mount_point: String,
    /// The config that protects it, if one does.
    pub protected_by: Option<AreaName>,
}

impl Area {
    pub fn is_protected(&self) -> bool {
        self.protected_by.is_some()
    }
}

fn normal(path: &str) -> &str {
    if path == "/" {
        path
    } else {
        path.trim_end_matches('/')
    }
}

/// The Btrfs areas of this machine, `/` first, with the config that protects each. `configs` pairs
/// every config with the subvolume it covers.
///
/// Snapper's own `.snapshots` directories are not areas, and a subvolume that is mounted twice is
/// one area.
pub fn areas(mounts: &[Mount], configs: &[(AreaName, String)]) -> Vec<Area> {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    let mut found: Vec<Area> = Vec::new();
    for mount in mounts.iter().filter(|m| m.fstype == "btrfs") {
        if mount.point.split('/').any(|part| part == ".snapshots") {
            continue;
        }
        if seen.contains(&(mount.source.as_str(), mount.root.as_str())) {
            continue;
        }
        seen.push((mount.source.as_str(), mount.root.as_str()));
        let protected_by = configs
            .iter()
            .find(|(_, location)| normal(location) == normal(&mount.point))
            .map(|(name, _)| name.clone());
        found.push(Area {
            mount_point: mount.point.clone(),
            protected_by,
        });
    }
    found.sort_by(|a, b| {
        (a.mount_point != "/", &a.mount_point).cmp(&(b.mount_point != "/", &b.mount_point))
    });
    found
}

/// The areas worth protecting that are not: where a mistake could not be undone.
pub fn gaps(areas: &[Area]) -> Vec<&Area> {
    areas
        .iter()
        .filter(|area| {
            !area.is_protected() && describe(&area.mount_point).worth == Worth::Recommended
        })
        .collect()
}

/// How much protecting an area is worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Worth {
    /// Where things go wrong, or where the user's own files are.
    Recommended,
    /// Up to the person: worth it if it holds something they would hate to lose.
    Optional,
    /// Programs recreate it, or it only grows: keeping old copies mostly wastes space.
    NotUseful,
}

/// What to tell a person about an area, in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// A name a person would use: `System`, `Home`.
    pub title: String,
    /// What is in it.
    pub holds: String,
    pub worth: Worth,
    /// Why protecting it is, or is not, worth it.
    pub reason: String,
}

/// The words for the area mounted at `mount_point`.
pub fn describe(mount_point: &str) -> Info {
    let known = |title: &str, holds: &str, worth, reason: &str| Info {
        title: title.to_owned(),
        holds: holds.to_owned(),
        worth,
        reason: reason.to_owned(),
    };
    match normal(mount_point) {
        "/" => known(
            "System",
            "the operating system itself: its programs, its settings in /etc, and everything that updates change",
            Worth::Recommended,
            "an update or a change to the system is the usual reason a computer stops working, and a snapshot lets you go back to before it",
        ),
        "/home" => known(
            "Home",
            "your own files: documents, downloads, pictures, and the settings of your applications",
            Worth::Recommended,
            "snapshots of the System area do not contain these files, so without this a mistake or a bad change here could not be undone",
        ),
        "/root" => known(
            "Administrator's home",
            "the files of the administrator account (root)",
            Worth::Optional,
            "only worth it if you keep files there",
        ),
        "/srv" => known(
            "Served data",
            "files this computer serves to others, such as a website",
            Worth::Optional,
            "worth it if you would hate to lose what is stored there",
        ),
        "/var/log" => known(
            "System logs",
            "the diary programs keep of what they did",
            Worth::NotUseful,
            "logs grow all the time, and putting old ones back rarely helps; snapshots of them mostly waste space",
        ),
        "/var/cache" | "/var/tmp" | "/tmp" => known(
            "Temporary files",
            "downloads and scratch files that programs make again when they need them",
            Worth::NotUseful,
            "programs recreate these on their own, so keeping old copies only wastes space",
        ),
        other => known(
            &folder_title(other),
            &format!("whatever is kept in {other}"),
            Worth::Optional,
            "worth it if it holds files you would hate to lose",
        ),
    }
}

/// The name of an area nobody has described: its folder's name with a capital, or `Storage`.
fn folder_title(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or_default();
    let mut chars = last.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Storage".to_owned(),
    }
}

/// A name for the config that will protect the area at `mount_point`: `root` for `/`, otherwise its
/// last directory name, made safe and made different from `taken`. `None` only if no valid name could
/// be found, which a directory name made of safe characters never causes.
pub fn suggest_name(mount_point: &str, taken: &[AreaName]) -> Option<AreaName> {
    let last = normal(mount_point).rsplit('/').next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = if normal(mount_point) == "/" {
        "root".to_owned()
    } else if cleaned.chars().all(|c| c == '_' || c == '-') {
        "data".to_owned()
    } else {
        cleaned
    };
    std::iter::once(base.clone())
        .chain((2..1000).map(|n| format!("{base}_{n}")))
        .filter_map(|candidate| AreaName::parse(&candidate).ok())
        .find(|name| !taken.contains(name))
}

#[cfg(test)]
mod tests {
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

    fn configs(pairs: &[(&str, &str)]) -> Vec<(AreaName, String)> {
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
        let none = areas(&mounts, &configs(&[("root", "/")]));
        assert_eq!(
            none.iter().map(Area::is_protected).collect::<Vec<_>>(),
            [true, false, false]
        );
        let both = areas(&mounts, &configs(&[("root", "/"), ("home", "/home/")]));
        assert_eq!(
            both[1].protected_by,
            Some(name("home")),
            "a trailing slash is the same directory"
        );
        // a config for some other directory does not protect /home
        let wrong = areas(&mounts, &configs(&[("docs", "/home/me/docs")]));
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
}
