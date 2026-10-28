// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Taking turns at changing things: the right to make a change is held in a file, so that two restores, or a
//! restore and a deletion, do not race each other.
//!
//! It guards snapall from snapall, not the disk from other programs: snapper and other tools do not know
//! of it. The file lives under `/run`, which is emptied at every start, so a crash cannot leave a lock
//! behind for ever; and a lock whose owner is gone is taken over.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use snapall_core::{Busy, Hold};

/// What is in the file: who holds it, what for, and since when.
fn contents(pid: u32, doing: &str, since: &str) -> String {
    format!("{pid}\n{since}\n{doing}\n")
}

fn read_owner(text: &str) -> Option<Busy> {
    let mut lines = text.lines();
    let pid = lines.next()?.trim().parse().ok()?;
    let since = lines.next().map(str::to_owned).filter(|s| !s.is_empty());
    let doing = lines.next().unwrap_or("a change").to_owned();
    Some(Busy { pid, doing, since })
}

/// The state letter in the text of `/proc/<pid>/stat` (`R`, `S`, `Z` for a zombie, ...). The command name
/// is in parentheses and may itself contain parentheses, so the state is what follows the *last* one.
fn state_in(stat: &str) -> Option<char> {
    stat.rsplit_once(')')?.1.trim_start().chars().next()
}

/// Whether the holder of the lock is still there. A process that exists and is not a zombie holds it,
/// unless its command line plainly shows it is something else (a pid can be reused). A command line that
/// is empty, which the kernel reports for a moment while a process is starting a program, or cannot be
/// read, is not that: it is no reason to take a lock from a process that may be about to need it.
pub(crate) fn is_running_snapall(pid: u32) -> bool {
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false; // no such process
    };
    if state_in(&stat) == Some('Z') {
        return false; // it has ended and only waits to be collected
    }
    match fs::read(format!("/proc/{pid}/cmdline")) {
        Ok(bytes) if !bytes.is_empty() => String::from_utf8_lossy(&bytes).contains("snapall"),
        _ => true,
    }
}

/// The lock, released when dropped.
struct Guard {
    path: PathBuf,
    pid: u32,
}

impl Drop for Guard {
    fn drop(&mut self) {
        // never remove a lock someone else took after ours was judged stale
        let ours = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| read_owner(&text))
            .is_some_and(|owner| owner.pid == self.pid);
        if ours {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Take the lock at `path`, or say who has it. `since` is written for the next one to read.
pub fn hold(path: &Path, doing: &str, since: &str) -> Result<Hold, Busy> {
    let pid = std::process::id();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    for _ in 0..2 {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(mut file) => {
                let _ = file.write_all(contents(pid, doing, since).as_bytes());
                return Ok(Box::new(Guard {
                    path: path.to_owned(),
                    pid,
                }));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let owner = fs::read_to_string(path)
                    .ok()
                    .and_then(|text| read_owner(&text));
                match owner {
                    Some(owner) if owner.pid != pid && is_running_snapall(owner.pid) => {
                        return Err(owner)
                    }
                    // its owner is gone (or it is unreadable, or it is us): take it over and try again
                    _ => {
                        let _ = fs::remove_file(path);
                    }
                }
            }
            // no way to keep a lock (a read-only /run): do not stop the change for it
            Err(_) => return Ok(Box::new(())),
        }
    }
    Ok(Box::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("snapall-lock-{}-{name}", std::process::id()))
            .join("change.lock")
    }

    #[test]
    fn a_second_change_is_refused_while_a_live_snapall_holds_the_lock_and_is_told_who_and_what() {
        // a real other process whose command line says snapall, standing in for another snapall
        let mut other = std::process::Command::new("bash")
            .args(["-c", "exec -a snapall-holder sleep 30"])
            .spawn()
            .unwrap();
        let path = lock_path("busy");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents(other.id(), "restoring Home", "12:03:00")).unwrap();

        let refused = hold(&path, "taking a snapshot", "now").expect_err("the lock is held");
        assert_eq!(refused.pid, other.id());
        assert_eq!(refused.doing, "restoring Home");
        assert_eq!(
            refused.to_string(),
            format!(
                "Another snapall is making a change right now (restoring Home, since 12:03:00, process {}). Wait for it to finish, then try again.",
                other.id()
            )
        );
        assert!(path.exists(), "the holder's lock was left alone");

        // once that process is gone, the lock is free to take
        other.kill().unwrap();
        other.wait().unwrap();
        assert!(hold(&path, "taking a snapshot", "now").is_ok());
    }

    #[test]
    fn a_lock_is_released_when_dropped_and_can_then_be_taken_again() {
        let path = lock_path("release");
        let first = hold(&path, "taking a snapshot", "now").unwrap();
        assert!(path.exists(), "the lock is a file while held");
        drop(first);
        assert!(!path.exists(), "and gone when let go");
        assert!(hold(&path, "again", "now").is_ok());
    }

    #[test]
    fn a_lock_whose_owner_is_gone_is_taken_over() {
        let path = lock_path("stale");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents(u32::MAX - 1, "restoring Home", "yesterday")).unwrap(); // no such process
        let taken = hold(&path, "taking a snapshot", "now");
        assert!(
            taken.is_ok(),
            "a crashed snapall must not block every later one"
        );
        assert_eq!(
            read_owner(&fs::read_to_string(&path).unwrap()).unwrap().pid,
            std::process::id()
        );
    }

    #[test]
    fn a_garbled_lock_file_does_not_block_anything() {
        let path = lock_path("garbled");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not a lock at all").unwrap();
        assert!(hold(&path, "taking a snapshot", "now").is_ok());
    }

    #[test]
    fn the_state_of_a_process_is_read_even_when_its_name_has_parentheses() {
        assert_eq!(state_in("4242 (snapall) S 1 4242 4242 0 -1"), Some('S'));
        assert_eq!(
            state_in("4242 (odd) name) Z 1 4242"),
            Some('Z'),
            "the last parenthesis ends the name"
        );
        assert_eq!(state_in("garbage"), None);
    }

    #[test]
    fn a_starting_process_with_no_command_line_yet_is_not_robbed_of_its_lock() {
        // this very process is running and is not a zombie: whatever its command line, a process that is
        // there is treated as the holder unless the command line says otherwise
        assert!(
            is_running_snapall(std::process::id())
                || !fs::read_to_string("/proc/self/cmdline")
                    .unwrap_or_default()
                    .is_empty()
        );
        assert!(
            !is_running_snapall(u32::MAX - 1),
            "a process that does not exist holds nothing"
        );
    }
}
