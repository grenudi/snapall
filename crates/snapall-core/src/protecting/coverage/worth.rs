// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a place on the disk is worth protecting, and what to call it.

use super::*;
use crate::vocabulary::domain::AreaName;

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
pub(super) fn folder_title(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or_default();
    let mut chars = last.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Storage".to_owned(),
    }
}

/// A name for the area that will protect the area at `mount_point`: `root` for `/`, otherwise its
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
