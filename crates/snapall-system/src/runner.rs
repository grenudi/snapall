// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Starting a change that outlives the program that asked for it, and asking it to stop.
//!
//! What runs is the ordinary `snapall` command line, given `--job ID`: the same code path a person types,
//! which takes the change lock and writes the journal itself. Where systemd is running it is started
//! through `systemd-run`, so the system keeps its log and it survives the end of a login session;
//! otherwise it is started as a process of its own, which goes on when the window closes.
//!
//! A stop is a small file the change looks for between its steps: nothing is signalled, so nothing is
//! ever interrupted in the middle of a step.

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where stop requests are kept. Under `/run`, which is emptied at every start.
pub const STOP_DIRECTORY: &str = "/run/snapall/stop";

/// A file name is made from an id; nothing else may become part of a path.
fn safe(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn stop_file(directory: &Path, id: &str) -> Option<PathBuf> {
    safe(id).then(|| directory.join(id))
}

pub fn request_stop(directory: &Path, id: &str) -> Result<(), String> {
    let file =
        stop_file(directory, id).ok_or_else(|| format!("`{id}` is not the name of a change"))?;
    fs::create_dir_all(directory).map_err(|e| format!("could not ask: {e}"))?;
    fs::write(&file, b"stop\n").map_err(|e| format!("could not ask: {e}"))
}

pub fn stop_requested(directory: &Path, id: &str) -> bool {
    stop_file(directory, id).is_some_and(|file| file.exists())
}

/// The request is answered (or moot) once the change has ended.
pub fn clear_stop(directory: &Path, id: &str) {
    if let Some(file) = stop_file(directory, id) {
        let _ = fs::remove_file(file);
    }
}

/// Whether the system was started with systemd, the way `sd_booted()` asks.
pub fn systemd_is_running() -> bool {
    Path::new("/run/systemd/system").exists()
}

/// The command that starts `program` with `arguments` as a change of its own.
pub fn command_line(
    program: &Path,
    id: &str,
    arguments: &[String],
    with_systemd: bool,
) -> (String, Vec<String>) {
    let program = program.display().to_string();
    // the options come first and the name with them: whatever follows a `--` is free text, and must
    // stay so (a snapshot's note may begin with a dash)
    let mut all = vec!["--no-dbus".to_owned()];
    let mut rest = arguments.iter().cloned();
    all.extend(rest.next());
    all.push("--job".to_owned());
    all.push(id.to_owned());
    all.extend(rest);
    if with_systemd {
        let mut line = vec![
            "--quiet".to_owned(),
            "--collect".to_owned(),
            "--no-block".to_owned(),
            format!("--unit=snapall-job-{id}"),
            "--setenv=SNAPALL_SOURCE=window".to_owned(),
            "--".to_owned(),
            program,
        ];
        line.extend(all);
        ("systemd-run".to_owned(), line)
    } else {
        (program, all)
    }
}

pub fn start(program: &Path, id: &str, arguments: &[String]) -> Result<(), String> {
    if !safe(id) {
        return Err(format!("`{id}` is not the name of a change"));
    }
    let with_systemd = systemd_is_running() && which("systemd-run");
    let (name, line) = command_line(program, id, arguments, with_systemd);
    let mut command = Command::new(&name);
    command
        .args(&line)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // a process group of its own: closing the terminal does not reach it
        .process_group(0);
    if !with_systemd {
        command.env("SNAPALL_SOURCE", "window");
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("could not start it: {e}"))?;
    if with_systemd {
        // systemd-run hands the work over and returns: its own failure is worth knowing
        let status = child.wait().map_err(|e| e.to_string())?;
        return status
            .success()
            .then_some(())
            .ok_or_else(|| "systemd-run did not start it".to_owned());
    }
    // not waited for: it is a process of its own now. A thread collects it so no zombie remains.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snapall-stop-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_stop_is_asked_seen_and_cleared_and_only_ever_for_the_change_named() {
        let dir = directory("asked");
        assert!(!stop_requested(&dir, "42-1-c"));
        request_stop(&dir, "42-1-c").unwrap();
        assert!(stop_requested(&dir, "42-1-c"));
        assert!(
            !stop_requested(&dir, "43-1-c"),
            "another change is not asked"
        );
        clear_stop(&dir, "42-1-c");
        assert!(!stop_requested(&dir, "42-1-c"));
    }

    #[test]
    fn nothing_but_a_plain_name_can_become_part_of_a_path() {
        let dir = directory("names");
        for bad in ["", "../etc/passwd", "a/b", "a b", "x;rm", "é"] {
            assert!(request_stop(&dir, bad).is_err(), "{bad:?}");
            assert!(!stop_requested(&dir, bad), "{bad:?}");
            assert!(start(Path::new("/bin/true"), bad, &[]).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_change_is_the_ordinary_command_line_given_its_name() {
        let args: Vec<String> = ["create", "--only", "home", "before theming"]
            .map(String::from)
            .to_vec();
        let (program, line) = command_line(Path::new("/usr/bin/snapall"), "9-1-w", &args, false);
        assert_eq!(program, "/usr/bin/snapall");
        assert_eq!(
            line,
            [
                "--no-dbus",
                "create",
                "--job",
                "9-1-w",
                "--only",
                "home",
                "before theming"
            ]
        );
        // a note that begins with a dash stays free text, after the `--`
        let dashed: Vec<String> = ["create", "--only", "home", "--", "--weird"]
            .map(String::from)
            .to_vec();
        let (_, line) = command_line(Path::new("/usr/bin/snapall"), "9-1-w", &dashed, false);
        assert_eq!(
            line,
            [
                "--no-dbus",
                "create",
                "--job",
                "9-1-w",
                "--only",
                "home",
                "--",
                "--weird"
            ]
        );
    }

    #[test]
    fn under_systemd_it_is_a_unit_named_for_the_change_that_keeps_its_log() {
        let args: Vec<String> = ["restore-all", "20261025T181500Z", "--yes"]
            .map(String::from)
            .to_vec();
        let (program, line) = command_line(Path::new("/usr/bin/snapall"), "9-1-w", &args, true);
        assert_eq!(program, "systemd-run");
        assert_eq!(
            &line[..4],
            [
                "--quiet",
                "--collect",
                "--no-block",
                "--unit=snapall-job-9-1-w"
            ]
        );
        assert!(line.contains(&"--setenv=SNAPALL_SOURCE=window".to_owned()));
        let at = line.iter().position(|a| a == "--").unwrap();
        assert_eq!(
            &line[at + 1..],
            [
                "/usr/bin/snapall",
                "--no-dbus",
                "restore-all",
                "--job",
                "9-1-w",
                "20261025T181500Z",
                "--yes"
            ]
        );
    }

    #[test]
    fn a_change_started_in_the_background_goes_on_and_is_a_process_of_its_own() {
        let marker = directory("alive").join("done");
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        // a stand-in for snapall that takes a moment and then leaves a mark
        let script = marker.parent().unwrap().join("fake-snapall");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nsleep 1\necho \"$SNAPALL_SOURCE $*\" > {}\n",
                marker.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        start(&script, "7-1-w", &["create".to_owned()]).unwrap();
        assert!(
            !marker.exists(),
            "start returns at once, it does not wait for the change"
        );
        for _ in 0..40 {
            if marker.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let said = fs::read_to_string(&marker).unwrap();
        assert_eq!(
            said.trim(),
            "window --no-dbus create --job 7-1-w",
            "it knows it came from the window, and its name"
        );
    }
}
