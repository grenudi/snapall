// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The journal on disk: one JSON object per line, appended. A line is written in a single call, which
//! the kernel does not interleave with another program's, so the window and a command writing at the same
//! moment cannot spoil each other's lines. The file is rotated when it grows, and a line that cannot be read
//! is skipped, never fatal: remembering is a convenience and must not stop a change.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use snapall_core::journal::{fold, Entry, Event, Outcome, Source};

pub const PATH: &str = "/var/lib/snapall/journal.jsonl";

/// Past this size the file is moved aside and a new one begun; one older file is kept.
const ROTATE_AT: u64 = 1024 * 1024;

/// A line is written whole or not at all, so it must stay small enough for one write.
const LONGEST_TEXT: usize = 1000;

fn shortened(text: &str) -> String {
    if text.chars().count() <= LONGEST_TEXT {
        text.to_owned()
    } else {
        let kept: String = text.chars().take(LONGEST_TEXT).collect();
        format!("{kept}...")
    }
}

pub fn to_line(event: &Event) -> String {
    let value = match event {
        Event::Started {
            id,
            at,
            what,
            source,
            pid,
        } => serde_json::json!({
            "event": "started", "id": id, "at": at, "what": shortened(what),
            "source": source.word(), "pid": pid,
        }),
        Event::Progress {
            id,
            at,
            done,
            of,
            area,
        } => serde_json::json!({
            "event": "progress", "id": id, "at": at, "done": done, "of": of, "area": shortened(area),
        }),
        Event::Finished {
            id,
            at,
            outcome,
            took_ms,
        } => {
            let (result, detail) = match outcome {
                Outcome::Done(text) => ("done", text),
                Outcome::Failed(text) => ("failed", text),
                Outcome::Cancelled(text) => ("cancelled", text),
            };
            serde_json::json!({
                "event": "finished", "id": id, "at": at, "took_ms": took_ms,
                "result": result, "detail": shortened(detail),
            })
        }
    };
    value.to_string()
}

pub fn parse_line(line: &str) -> Option<Event> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let text = |key: &str| value.get(key)?.as_str().map(str::to_owned);
    let id = text("id")?;
    let at = text("at")?;
    match value.get("event")?.as_str()? {
        "started" => Some(Event::Started {
            id,
            at,
            what: text("what")?,
            source: Source::from_word(&text("source")?)?,
            pid: u32::try_from(value.get("pid")?.as_u64()?).ok()?,
        }),
        "progress" => Some(Event::Progress {
            id,
            at,
            done: u32::try_from(value.get("done")?.as_u64()?).ok()?,
            of: u32::try_from(value.get("of")?.as_u64()?).ok()?,
            area: text("area")?,
        }),
        "finished" => {
            let detail = text("detail").unwrap_or_default();
            // lines written before a change could be cancelled said only whether it was all right
            let result = match (
                text("result").as_deref(),
                value.get("ok").and_then(serde_json::Value::as_bool),
            ) {
                (Some("done"), _) | (None, Some(true)) => Outcome::Done(detail),
                (Some("failed"), _) | (None, Some(false)) => Outcome::Failed(detail),
                (Some("cancelled"), _) => Outcome::Cancelled(detail),
                _ => return None,
            };
            Some(Event::Finished {
                id,
                at,
                outcome: result,
                took_ms: value.get("took_ms")?.as_u64()?,
            })
        }
        _ => None,
    }
}

/// Add an event to the journal at `path`. Any failure is ignored: see the module's note.
pub fn append(path: &Path, event: &Event) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(path).is_ok_and(|meta| meta.len() > ROTATE_AT) {
        let _ = fs::rename(path, older(path));
    }
    let mut line = to_line(event);
    line.push('\n');
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

fn older(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

/// Every readable event, the older file first.
pub fn read_events(path: &Path) -> Vec<Event> {
    [older(path), path.to_owned()]
        .iter()
        .filter_map(|file| fs::read_to_string(file).ok())
        .flat_map(|text| text.lines().filter_map(parse_line).collect::<Vec<_>>())
        .collect()
}

/// The changes, newest first, at most `limit` of them.
pub fn history(path: &Path, limit: usize, is_alive: impl Fn(u32) -> bool) -> Vec<Entry> {
    let mut entries = fold(&read_events(path), is_alive);
    entries.truncate(limit);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::journal::Status;

    fn dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("snapall-journal-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn started(id: &str, pid: u32) -> Event {
        Event::Started {
            id: id.into(),
            at: "2026-10-25 18:15:00".into(),
            what: "Took a snapshot of home".into(),
            source: Source::Window,
            pid,
        }
    }

    fn finished(id: &str, outcome: Result<&str, &str>) -> Event {
        Event::Finished {
            id: id.into(),
            at: "2026-10-25 18:15:03".into(),
            outcome: outcome.map(str::to_owned).map_err(str::to_owned).into(),
            took_ms: 3000,
        }
    }

    #[test]
    fn an_event_survives_the_trip_through_a_line_even_with_quotes_newlines_and_unicode() {
        for event in [
            started("a", 7),
            finished("a", Ok("")),
            finished("b", Err("it said \"no\"\nand then\ttabs, and ünïcödé ✓")),
            Event::Started {
                id: "c".into(),
                at: "t".into(),
                what: "Restored /home/a b/c.txt from snapshot #3".into(),
                source: Source::Command,
                pid: 1,
            },
        ] {
            let line = to_line(&event);
            assert!(!line.contains('\n'), "one event is one line: {line:?}");
            assert_eq!(parse_line(&line), Some(event));
        }
    }

    #[test]
    fn what_cannot_be_read_is_skipped_and_never_fatal() {
        for bad in [
            "",
            "not json",
            "{}",
            r#"{"event":"started","id":"a"}"#,
            r#"{"event":"mystery","id":"a","at":"t"}"#,
            r#"{"event":"started","id":"a","at":"t","what":"x","source":"alien","pid":1}"#,
        ] {
            assert_eq!(parse_line(bad), None, "{bad:?}");
        }
        let path = dir("skip").join("j.jsonl");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(
                "garbage\n{}\n\n{}\n{{broken",
                to_line(&started("a", 1)),
                to_line(&finished("a", Ok("ok")))
            ),
        )
        .unwrap();
        let entries = history(&path, 10, |_| true);
        assert_eq!((entries.len(), entries[0].status), (1, Status::Done));
    }

    #[test]
    fn written_events_come_back_as_the_changes_they_made_newest_first() {
        let path = dir("round").join("j.jsonl");
        for event in [
            started("a", std::process::id()),
            finished("a", Ok("done")),
            started("b", std::process::id()),
            finished("b", Err("boom")),
        ] {
            append(&path, &event);
        }
        let entries = history(&path, 10, |_| true);
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.id.as_str(), e.status))
                .collect::<Vec<_>>(),
            [("b", Status::Failed), ("a", Status::Done)]
        );
        assert_eq!(history(&path, 1, |_| true).len(), 1, "the limit applies");
    }

    #[test]
    fn two_programs_writing_at_the_same_time_cannot_spoil_each_others_lines() {
        let path = dir("race").join("j.jsonl");
        let writers: Vec<_> = (0..4)
            .map(|w| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for n in 0..150 {
                        append(&path, &started(&format!("w{w}-{n}"), 1));
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 600);
        assert!(
            text.lines().all(|line| parse_line(line).is_some()),
            "every line is whole"
        );
    }

    #[test]
    fn a_big_journal_is_rotated_and_the_older_file_is_still_read() {
        let path = dir("rotate").join("j.jsonl");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, " ".repeat(usize::try_from(ROTATE_AT).unwrap() + 10)).unwrap();
        append(&path, &started("before", 1));
        assert!(older(&path).exists(), "the big file was moved aside");
        append(&path, &finished("before", Ok("done")));
        let entries = history(&path, 10, |_| true);
        assert_eq!(entries.len(), 1, "the start and the end are one change");
    }

    #[test]
    fn a_change_whose_process_is_gone_is_stopped_and_one_whose_process_runs_is_running() {
        let path = dir("alive").join("j.jsonl");
        append(&path, &started("dead", u32::MAX - 1));
        append(&path, &started("live", std::process::id()));
        let entries = history(&path, 10, crate::lock::is_running_snapall);
        let by = |id: &str| entries.iter().find(|e| e.id == id).unwrap().status;
        assert_eq!(
            (by("dead"), by("live")),
            (Status::Interrupted, Status::Running)
        );
    }

    #[test]
    fn a_very_long_message_is_cut_so_that_the_line_stays_whole() {
        let line = to_line(&finished("a", Err(&"x".repeat(5000))));
        assert!(line.len() < 1200, "{}", line.len());
        assert!(
            matches!(parse_line(&line), Some(Event::Finished { outcome: Outcome::Failed(text), .. }) if text.ends_with("..."))
        );
    }

    #[test]
    fn progress_and_a_cancelled_ending_survive_the_trip_and_old_lines_are_still_read() {
        let progress = Event::Progress {
            id: "a".into(),
            at: "t".into(),
            done: 1,
            of: 3,
            area: "home".into(),
        };
        assert_eq!(parse_line(&to_line(&progress)), Some(progress));
        let cancelled = Event::Finished {
            id: "a".into(),
            at: "t".into(),
            outcome: Outcome::Cancelled("1 of 3 areas".into()),
            took_ms: 5,
        };
        assert_eq!(parse_line(&to_line(&cancelled)), Some(cancelled));
        // what the previous version wrote: a plain yes or no
        let old_ok = r#"{"event":"finished","id":"a","at":"t","took_ms":9,"ok":true,"detail":"2 snapshots taken"}"#;
        assert_eq!(
            parse_line(old_ok),
            Some(Event::Finished {
                id: "a".into(),
                at: "t".into(),
                outcome: Outcome::Done("2 snapshots taken".into()),
                took_ms: 9
            })
        );
        let old_no =
            r#"{"event":"finished","id":"a","at":"t","took_ms":9,"ok":false,"detail":"boom"}"#;
        assert!(matches!(
            parse_line(old_no),
            Some(Event::Finished {
                outcome: Outcome::Failed(_),
                ..
            })
        ));
    }
}
