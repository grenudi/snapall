// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! One line per event, as text for people or JSON for machines.

use std::io::Write;
use std::sync::Mutex;
use std::time::SystemTime;

use serde_json::{json, Map, Value};
use snapall_core::{Event, Observer};

use crate::clock::rfc3339_utc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `2026-10-23T05:43:12Z INFO  snapshot_created config=home snapshot=14 took_ms=212 description="x"`
    Text,
    /// One JSON object per line (JSON Lines), keys in alphabetical order.
    Json,
}

/// Writes each event as a line to `writer`.
///
/// A line that cannot be written (full disk, closed pipe) is dropped: observing must never fail
/// the operation observed.
pub struct Lines<W: Write> {
    writer: Mutex<W>,
    format: Format,
    clock: fn() -> SystemTime,
}

impl<W: Write> Lines<W> {
    pub fn new(writer: W, format: Format) -> Self {
        Self {
            writer: Mutex::new(writer),
            format,
            clock: SystemTime::now,
        }
    }

    /// Replace the clock (tests, and anyone who wants a different time source).
    #[must_use]
    pub fn with_clock(mut self, clock: fn() -> SystemTime) -> Self {
        self.clock = clock;
        self
    }
}

impl<W: Write> Observer for Lines<W> {
    fn observe(&self, event: &Event) {
        let line = render(&rfc3339_utc((self.clock)()), event, self.format);
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.flush();
        }
    }
}

/// The fields of an event as (name, value) pairs, in the order a person reads them.
fn fields(event: &Event) -> Vec<(&'static str, Value)> {
    let mut fields: Vec<(&'static str, Value)> = Vec::new();
    if let Some(area) = event.area() {
        fields.push(("config", json!(area.as_str())));
    }
    match event {
        Event::AreasListed {
            result: Ok(count), ..
        }
        | Event::SnapshotsListed {
            result: Ok(count), ..
        } => fields.push(("count", json!(count))),
        Event::SnapshotCreated {
            description,
            point,
            result,
            ..
        } => {
            if let Ok(id) = result {
                fields.push(("snapshot", json!(id.get())));
            }
            fields.push(("description", json!(description)));
            if let Some(point) = point {
                fields.push(("point", json!(point)));
            }
        }
        Event::SnapshotDeleted { id, .. } => fields.push(("snapshot", json!(id.get()))),
        Event::ChangesUndone {
            id, path, result, ..
        } => {
            fields.push(("snapshot", json!(id.get())));
            if let Some(path) = path {
                fields.push(("path", json!(path)));
            }
            if let Ok(summary) = result {
                fields.push(("created", json!(summary.created)));
                fields.push(("modified", json!(summary.modified)));
                fields.push(("deleted", json!(summary.deleted)));
            }
        }
        Event::AreaChanged { change, .. } => fields.push(("change", json!(change))),
        Event::InstallStepRun { command, .. } => fields.push(("command", json!(command))),
        Event::AreasListed { .. } | Event::SnapshotsListed { .. } => {}
    }
    fields.push((
        "took_ms",
        json!(u64::try_from(event.took().as_millis()).unwrap_or(u64::MAX)),
    ));
    if let Some(error) = event.error() {
        fields.push(("error", json!(error)));
    }
    fields
}

fn level(event: &Event) -> &'static str {
    if event.is_failure() {
        "error"
    } else {
        "info"
    }
}

/// A complete line, newline included.
pub(crate) fn render(time: &str, event: &Event, format: Format) -> String {
    match format {
        Format::Json => {
            let mut object = Map::new();
            object.insert("time".into(), json!(time));
            object.insert("level".into(), json!(level(event)));
            object.insert("event".into(), json!(event.name()));
            for (key, value) in fields(event) {
                object.insert(key.into(), value);
            }
            format!("{}\n", Value::Object(object))
        }
        Format::Text => {
            let mut line = format!("{time} {:<5} {}", level(event).to_uppercase(), event.name());
            for (key, value) in fields(event) {
                let text = match &value {
                    // Quote free text so a value with spaces cannot be mistaken for more fields.
                    Value::String(s) if key != "config" => format!("{s:?}"),
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                line.push_str(&format!(" {key}={text}"));
            }
            line.push('\n');
            line
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::{Duration, UNIX_EPOCH};

    use snapall_core::{AreaName, SnapshotId};

    use super::*;

    fn fixed_clock() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_761_187_392)
    }

    fn created(area: &str, description: &str, result: Result<u64, &str>) -> Event {
        Event::SnapshotCreated {
            area: AreaName::parse(area).unwrap(),
            description: description.to_owned(),
            result: result
                .map(|n| SnapshotId::new(n).unwrap())
                .map_err(str::to_owned),
            took: Duration::from_millis(212),
            point: None,
        }
    }

    /// A writer the test can read back.
    #[derive(Clone, Default)]
    struct Buffer(Arc<StdMutex<Vec<u8>>>);

    impl Write for Buffer {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Buffer {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    #[test]
    fn a_json_line_is_one_parseable_object_with_the_documented_fields() {
        let line = render(
            "2026-10-23T05:43:12Z",
            &created("home", "before theming", Ok(14)),
            Format::Json,
        );
        assert!(line.ends_with('\n') && line.matches('\n').count() == 1);
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            value,
            json!({
                "time": "2026-10-23T05:43:12Z", "level": "info", "event": "snapshot_created",
                "config": "home", "snapshot": 14, "description": "before theming", "took_ms": 212
            })
        );
    }

    #[test]
    fn a_failure_is_level_error_and_carries_its_message() {
        let line = render(
            "t",
            &created("root", "x", Err("snapper said no")),
            Format::Json,
        );
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["level"], "error");
        assert_eq!(value["error"], "snapper said no");
        assert!(value.get("snapshot").is_none(), "no id was made");
    }

    #[test]
    fn text_lines_are_readable_and_quote_free_text() {
        let line = render(
            "2026-10-23T05:43:12Z",
            &created("home", "before theming", Ok(14)),
            Format::Text,
        );
        assert_eq!(
            line,
            "2026-10-23T05:43:12Z INFO  snapshot_created config=home snapshot=14 description=\"before theming\" took_ms=212\n"
        );
        let failed = render(
            "t",
            &created("home", "x", Err("boom \"bad\"")),
            Format::Text,
        );
        assert!(failed.contains("ERROR snapshot_created"), "{failed}");
        assert!(failed.contains(r#"error="boom \"bad\"""#), "{failed}");
    }

    #[test]
    fn newlines_in_a_message_cannot_forge_a_second_log_line() {
        let hostile = created("home", "x\nERROR forged line", Err("a\nb"));
        for format in [Format::Text, Format::Json] {
            let line = render("t", &hostile, format);
            assert_eq!(line.matches('\n').count(), 1, "{format:?}: {line:?}");
        }
    }

    #[test]
    fn lists_report_a_count_and_installs_report_the_command() {
        let listed = Event::AreasListed {
            result: Ok(3),
            took: Duration::ZERO,
        };
        let value: Value = serde_json::from_str(&render("t", &listed, Format::Json)).unwrap();
        assert_eq!(value["count"], 3);
        let install = Event::InstallStepRun {
            command: "apt-get install -y snapper".into(),
            result: Ok(()),
            took: Duration::from_millis(5),
        };
        let value: Value = serde_json::from_str(&render("t", &install, Format::Json)).unwrap();
        assert_eq!(value["command"], "apt-get install -y snapper");
    }

    #[test]
    fn the_observer_stamps_each_line_from_its_clock() {
        let buffer = Buffer::default();
        let observer = Lines::new(buffer.clone(), Format::Text).with_clock(fixed_clock);
        observer.observe(&created("home", "a", Ok(1)));
        observer.observe(&created("root", "b", Ok(2)));
        let text = buffer.text();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines.iter().all(|l| l.starts_with("2025-10-23T02:43:12Z ")),
            "{lines:?}"
        );
    }

    #[test]
    fn a_sink_that_fails_does_not_fail_the_operation_or_panic() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "gone"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("gone"))
            }
        }
        let observer = Lines::new(Broken, Format::Json);
        observer.observe(&created("home", "a", Ok(1)));
    }
}
