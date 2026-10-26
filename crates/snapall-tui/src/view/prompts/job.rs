// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The controls of one background job: what it is, what the machine says about it, and the keys that
//! control it, each beside the command a person could type instead.

use crate::model::{job_of, KEYS, RUN_NOW_KEY};
use crate::view::*;

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

pub(crate) fn job_spec(unit: &str, state: &State) -> ModalSpec {
    let Some(job) = job_of(unit) else {
        return ModalSpec {
            title: " Background job ".into(),
            lines: vec![Line::from(format!("{unit} is not a job snapall knows."))],
            color: ACCENT,
            width: 78,
            height: 7,
            buttons: vec![close_button()],
        };
    };
    let facts = state
        .health
        .data
        .as_ref()
        .and_then(|data| data.facts.job(&job));
    let heading = |text: &str| Line::styled(text.to_owned(), Style::new().fg(ACCENT).bold());
    let mut lines = vec![Line::from(capital(&format!("{}.", job.purpose)))];
    let description = facts
        .and_then(|f| f.details.as_ref())
        .and_then(|d| d.description.as_deref());
    lines.push(Line::from(match description {
        Some(text) => format!("{} (\"{text}\")", job.unit),
        None => job.unit.to_owned(),
    }));
    if let Some(facts) = facts {
        let schedule = facts
            .details
            .as_ref()
            .map(|d| d.schedule())
            .unwrap_or_default();
        if !schedule.is_empty() {
            lines.push(Line::from(format!("Runs {}.", schedule.join(", "))));
        }
        let (enabled, active) = (
            facts.state.enabled.as_deref().unwrap_or("unknown"),
            facts.state.active.as_deref().unwrap_or("unknown"),
        );
        lines.push(Line::from(format!(
            "Now: {enabled} at start, {active} at the moment."
        )));
    }
    lines.push(Line::from(""));
    lines.push(heading(
        "Press a key to do it here, or type the command yourself:",
    ));
    for (key, action) in KEYS {
        lines.push(Line::from(vec![
            Span::styled(format!(" {key}  "), Style::new().fg(Color::Yellow).bold()),
            Span::raw(format!("{:<24}", action.label())),
            Span::styled(job.command_for(action), Style::new().fg(DIM)),
        ]));
    }
    if let Some(command) = job.run_now_command() {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {RUN_NOW_KEY}  "),
                Style::new().fg(Color::Yellow).bold(),
            ),
            Span::raw(format!("{:<24}", "Run it right now")),
            Span::styled(command, Style::new().fg(DIM)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(heading("To look for yourself:"));
    for command in job.commands().into_iter().take(2) {
        lines.push(Line::from(vec![
            Span::raw(format!(" {:<28}", command.what)),
            Span::styled(command.line, Style::new().fg(DIM)),
        ]));
    }
    let width = 80;
    // the box, its border and the line for the buttons, plus every line as it wraps
    let height = (rows_needed(&lines, width) + 4).clamp(10, 21);
    ModalSpec {
        title: format!(" {} ", job.title),
        lines,
        color: Color::Yellow,
        width,
        height,
        buttons: vec![close_button()],
    }
}
