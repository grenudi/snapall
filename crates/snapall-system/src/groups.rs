// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The groups file: plain text under `/etc/snapall`, read and written whole. A missing file is simply no
//! groups; a file that cannot be read is an error that names it and the line, so a typo is never a group
//! that quietly went missing. It is written to a new file and moved into place, so a crash cannot leave
//! half of it.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

use snapall_core::Groups;

pub const PATH: &str = "/etc/snapall/groups";

pub fn read(path: &Path) -> Result<Groups, String> {
    match fs::read_to_string(path) {
        Ok(text) => Groups::parse(&text).map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Groups::default()),
        Err(error) => Err(plain(path, &error)),
    }
}

pub fn write(path: &Path, groups: &Groups) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| "the groups file has no folder".to_owned())?;
    fs::create_dir_all(directory).map_err(|e| plain(path, &e))?;
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".new-{}", std::process::id()));
    let temporary = std::path::PathBuf::from(name);
    let written = fs::File::create(&temporary)
        .and_then(|mut file| {
            file.write_all(groups.to_text().as_bytes())
                .and_then(|()| file.sync_all())
        })
        .and_then(|()| fs::rename(&temporary, path));
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(plain(path, &error));
    }
    Ok(())
}

fn plain(path: &Path, error: &std::io::Error) -> String {
    if error.kind() == ErrorKind::PermissionDenied {
        format!(
            "{} cannot be changed without administrator rights: start snapall with sudo",
            path.display()
        )
    } else {
        format!("{}: {error}", path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::{AreaName, GroupName};

    fn directory(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("snapall-groups-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn sample() -> Groups {
        let mut groups = Groups::default();
        groups
            .set(
                GroupName::parse("theming").unwrap(),
                ["home", "data"]
                    .map(|a| AreaName::parse(a).unwrap())
                    .to_vec(),
            )
            .unwrap();
        groups
    }

    #[test]
    fn no_file_is_no_groups_and_what_is_written_is_what_is_read() {
        let path = directory("round").join("etc/groups");
        assert_eq!(
            read(&path).unwrap(),
            Groups::default(),
            "no file, no groups"
        );
        write(&path, &sample()).unwrap();
        assert_eq!(read(&path).unwrap(), sample());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# snapall groups: a name, then the areas it covers\ntheming: home, data\n"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_naming_the_file_and_the_line() {
        let path = directory("bad").join("groups");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "theming: home\nthis is not a group\n").unwrap();
        let said = read(&path).unwrap_err();
        assert!(said.contains("groups") && said.contains("line 2"), "{said}");
    }

    #[test]
    fn writing_leaves_no_half_file_behind_and_replaces_the_old_one_whole() {
        let path = directory("atomic").join("groups");
        write(&path, &sample()).unwrap();
        let mut other = Groups::default();
        other
            .set(
                GroupName::parse("games").unwrap(),
                vec![AreaName::parse("data").unwrap()],
            )
            .unwrap();
        write(&path, &other).unwrap();
        assert_eq!(read(&path).unwrap(), other);
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".new-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn a_folder_that_cannot_be_written_is_said_plainly_and_nothing_is_left() {
        // a path under a file cannot be a folder: the nearest thing to refusing, whoever runs the test
        let file = directory("refuse");
        fs::create_dir_all(&file).unwrap();
        let blocker = file.join("blocker");
        fs::write(&blocker, "x").unwrap();
        let said = write(&blocker.join("groups"), &sample()).unwrap_err();
        assert!(said.contains("groups"), "{said}");
    }
}
