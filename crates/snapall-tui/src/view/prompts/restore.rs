// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The restore prompt: what would change, and the offer to put the whole restore point back.

use crate::view::*;

/// The restore prompt, and, when the snapshot is one of several taken together, the offer to put all
/// of them back.
pub(crate) fn restore_spec(
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    stage: &RestoreStage,
    state: &State,
) -> ModalSpec {
    let spec = restore_stage_spec(config, id, path, stage, state);
    if !matches!(stage, RestoreStage::Ready(_)) {
        return spec;
    }
    match &state.point_offer {
        Some((c, i, offer)) if c == config && *i == id && offer.areas_to_change() > 0 => {
            with_point(spec, offer)
        }
        _ => spec,
    }
}

/// Add the restore point's offer to a prompt: what the point is, and the button that puts it all back.
pub(crate) fn with_point(mut spec: ModalSpec, offer: &crate::model::PointOffer) -> ModalSpec {
    let areas: Vec<&str> = offer
        .point
        .members
        .iter()
        .map(|m| m.config.as_str())
        .collect();
    let k = offer.areas_to_change();
    spec.lines.push(Line::from(""));
    spec.lines.push(Line::from(format!(
        "This snapshot is part of restore point {} ({}), taken in {} areas: {}.",
        offer.point.point,
        offer.point.description,
        areas.len(),
        areas.join(", ")
    )));
    spec.lines.push(Line::styled(
        format!("Press A to put all {k} that differ back at once."),
        Style::new().fg(Color::Yellow),
    ));
    spec.lines.push(Line::styled(
        "The running system is left alone.",
        Style::new().fg(Color::Yellow),
    ));
    spec.height = (rows_needed(&spec.lines, spec.width) + 4).clamp(10, MAX_PROMPT_HEIGHT);
    let last = spec.buttons.len().saturating_sub(1);
    spec.buttons
        .insert(last, (Button::RestoreAll, format!("A: Restore all {k}")));
    spec
}

/// The restore prompt: looking, failed, the system root (refused), nothing to restore, or ready.
pub(crate) fn restore_stage_spec(
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    stage: &RestoreStage,
    state: &State,
) -> ModalSpec {
    let title = format!(" Restore {config} to #{id} ");
    match stage {
        RestoreStage::Loading => ModalSpec {
            title,
            lines: vec![Line::styled(
                "Looking at what would change...",
                Style::new().fg(DIM),
            )],
            color: Color::Yellow,
            width: 72,
            height: 8,
            buttons: vec![cancel_button()],
        },
        RestoreStage::Failed(error) => ModalSpec {
            title,
            lines: vec![Line::styled(error.clone(), Style::new().fg(Color::Red))],
            color: Color::Red,
            width: 72,
            height: 9,
            buttons: vec![close_button()],
        },
        RestoreStage::Ready(preview) if preview.is_system_root && path.is_none() => ModalSpec {
            title: " Restoring the system itself ".into(),
            lines: vec![
                Line::from(format!(
                    "{config} is the system root, so snapall will not put it back while it runs."
                )),
                Line::from(""),
                Line::from(state.context.restore_route.clone()),
            ],
            color: Color::Yellow,
            width: 78,
            height: 14,
            buttons: vec![close_button()],
        },
        RestoreStage::Ready(preview) if preview.changes.is_empty() => ModalSpec {
            title,
            lines: vec![Line::from(format!(
                "{config} already matches #{id}: there is nothing to restore."
            ))],
            color: ACCENT,
            width: 72,
            height: 8,
            buttons: vec![close_button()],
        },
        RestoreStage::Ready(preview) => {
            let lines = restore_summary(config, id, path, &preview.changes);
            let height = (rows_needed(&lines, 78) + 4).clamp(10, MAX_PROMPT_HEIGHT);
            ModalSpec {
                title,
                lines,
                color: Color::Yellow,
                width: 78,
                height,
                buttons: vec![confirm_button("Restore"), cancel_button()],
            }
        }
    }
}

/// What a restore would do: the counts, the first few paths with a verb each, and the safety net.
pub(crate) fn restore_summary(
    config: &ConfigName,
    id: SnapshotId,
    path: Option<&str>,
    changes: &[snapall_core::Change],
) -> Vec<Line<'static>> {
    let (changed, added, removed) = restore_counts(changes);
    let mut lines = vec![
        Line::from(format!(
            "Put {} back to how #{id} had it: {changed} reverted, {added} removed, {removed} brought back.",
            path.unwrap_or(config.as_str())
        )),
        Line::from(""),
    ];
    for change in changes.iter().take(6) {
        let verb = match change.kind {
            ChangeKind::Changed => "revert    ",
            ChangeKind::Added => "remove    ",
            ChangeKind::Removed => "bring back",
        };
        lines.push(Line::from(vec![
            Span::styled(verb, Style::new().fg(ACCENT)),
            Span::raw(format!(" {}", change.path)),
        ]));
    }
    if changes.len() > 6 {
        lines.push(Line::styled(
            format!("... and {} more", changes.len() - 6),
            Style::new().fg(DIM),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "The current state is snapshotted first, so this can be undone.",
        Style::new().fg(Color::Yellow),
    ));
    lines.push(Line::from(""));
    lines.push(Line::styled(explain::RESTORE_CAUTION, Style::new().fg(DIM)));
    lines
}
