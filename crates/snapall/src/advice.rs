// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What to tell someone whose file no snapshot can contain: why, and the command that fixes it.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use snapall_core::HistoryError;

/// The directory at the top of the subvolume that holds `path`: the highest ancestor of the file's
/// directory that is still on the same device. Btrfs gives each subvolume its own device number, so
/// where the number changes is where the subvolume begins, whether it is mounted there (`/home`) or
/// only nested.
pub fn subvolume_root(path: &Path) -> PathBuf {
    let device = |p: &Path| std::fs::metadata(p).map(|m| m.dev()).ok();
    let start = path
        .parent()
        .and_then(|dir| dir.ancestors().find(|a| a.exists()))
        .unwrap_or(Path::new("/"));
    let Some(wanted) = device(start) else {
        return PathBuf::from("/");
    };
    let mut top = start;
    for ancestor in start.ancestors().skip(1) {
        if device(ancestor) != Some(wanted) {
            break;
        }
        top = ancestor;
    }
    top.to_owned()
}

/// A config name for a subvolume at `root`: its last directory name with anything snapper's names do
/// not take replaced, `root` for `/`.
pub fn config_name_for(root: &Path) -> String {
    let last = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
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
    if cleaned.chars().all(|c| c == '_') {
        if root == Path::new("/") {
            "root".to_owned()
        } else {
            "data".to_owned()
        }
    } else {
        cleaned
    }
}

/// The error, and where it is one of the two that mean "no snapshot can contain this file", the
/// command that gives it one.
pub fn explain(error: &HistoryError, path: &str) -> String {
    if !matches!(
        error,
        HistoryError::OtherSubvolume { .. } | HistoryError::NoConfig { .. }
    ) {
        return error.to_string();
    }
    let root = subvolume_root(Path::new(path));
    let name = config_name_for(&root);
    format!(
        "{error}\n\nTo give it snapshots, make a snapper config for the subvolume that holds it:\n    sudo snapper -c {name} create-config {root}\n    sudo snapper -c {name} set-config TIMELINE_CREATE=no   (so only snapall makes its snapshots)\nsnapall then snapshots it together with your other configs.",
        root = root.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::ConfigName;

    #[test]
    fn a_config_name_is_made_from_the_directory_and_always_valid() {
        for (root, expected) in [
            ("/home", "home"),
            ("/", "root"),
            ("/srv/my data", "my_data"),
            ("/mnt/games-2", "games-2"),
            ("/mnt/\u{e4}\u{f6}\u{fc}", "data"),
        ] {
            let name = config_name_for(Path::new(root));
            assert_eq!(name, expected, "{root}");
            assert!(
                ConfigName::parse(&name).is_ok(),
                "`{name}` must be a name snapper accepts"
            );
        }
    }

    #[test]
    fn the_subvolume_root_is_where_the_device_changes() {
        // /proc is a filesystem of its own, so a file in it belongs to a "subvolume" rooted at /proc,
        // not at /
        assert_eq!(
            subvolume_root(Path::new("/proc/self/environ")),
            Path::new("/proc")
        );
        // whatever the machine's layout, the answer is an ancestor, and the one above it differs
        let tmp = std::env::temp_dir().join("snapall-advice-x");
        let root = subvolume_root(&tmp);
        assert!(tmp.starts_with(&root), "{root:?} is an ancestor of {tmp:?}");
        if let (Some(above), Ok(top)) = (root.parent(), std::fs::metadata(&root)) {
            assert_ne!(
                std::fs::metadata(above).map(|m| m.dev()).ok(),
                Some(top.dev()),
                "the parent is on another device"
            );
        }
    }

    #[test]
    fn only_the_two_no_snapshot_can_contain_it_errors_get_the_command() {
        let root = ConfigName::parse("root").unwrap();
        let other = HistoryError::OtherSubvolume {
            path: "/proc/self/environ".into(),
            config: root,
            subvolume: "/".into(),
        };
        let text = explain(&other, "/proc/self/environ");
        assert!(
            text.contains("sudo snapper -c proc create-config /proc"),
            "{text}"
        );
        assert!(text.contains("TIMELINE_CREATE=no"), "{text}");
        let none = explain(
            &HistoryError::NoConfig {
                path: "/x/y".into(),
            },
            "/proc/self/environ",
        );
        assert!(none.contains("create-config"), "{none}");
        let bad = explain(&HistoryError::BadPath { path: "rel".into() }, "rel");
        assert!(
            !bad.contains("create-config"),
            "an unrelated error is passed through: {bad}"
        );
    }
}
