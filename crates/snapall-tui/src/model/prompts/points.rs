// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restore points: the list of them, the preview of the chosen one, and putting one back.

use crate::model::*;
use snapall_core::Act;

impl State {
    /// The restore point the points screen has the cursor on.
    pub fn chosen_point(&self) -> Option<&snapall_core::RestorePoint> {
        match &self.modal {
            Some(Modal::Points { index }) => self.points.as_ref()?.get(*index),
            _ => None,
        }
    }
}

/// The restore points arrived: show them, and look up what the first would do.
pub(crate) fn on_points_loaded(
    state: &mut State,
    result: Result<Vec<snapall_core::RestorePoint>, String>,
) -> Vec<Effect> {
    match result {
        Err(why) => {
            state.modal = None;
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
        Ok(points) => {
            let first = points.first().cloned();
            state.points = Some(points);
            first
                .map(|point| vec![Effect::LoadPointPreview { point }])
                .unwrap_or_default()
        }
    }
}

/// The restore points screen: Up and Down choose, Enter or `A` puts every area of the chosen one back,
/// Esc leaves. Nothing happens to a point whose areas already match.
pub(crate) fn browsing_points(state: &mut State, key: Key, index: usize) -> Reaction {
    let count = state.points.as_ref().map_or(0, Vec::len);
    let index = index.min(count.saturating_sub(1));
    let moved = |to: usize, state: &mut State| {
        state.point_preview = None;
        let effects = state
            .points
            .as_ref()
            .and_then(|points| points.get(to))
            .map(|point| {
                vec![Effect::LoadPointPreview {
                    point: point.clone(),
                }]
            })
            .unwrap_or_default();
        Reaction::stays_and_starts(Modal::Points { index: to }, effects)
    };
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Down | Key::Char('j') | Key::Tab if index + 1 < count => moved(index + 1, state),
        Key::Up | Key::Char('k') if index > 0 => moved(index - 1, state),
        Key::Enter | Key::Char('A') => {
            let chosen = state
                .points
                .as_ref()
                .and_then(|points| points.get(index))
                .cloned();
            match (chosen, &state.point_preview) {
                (Some(point), Some(preview))
                    if preview.point.point == point.point && preview.areas_to_change() > 0 =>
                {
                    let lines = vec![
                        format!(
                            "Put {} area(s) back to restore point {}.",
                            preview.areas_to_change(),
                            point.point
                        ),
                        "Each area's current state is saved first, so this can be rolled back: the Activity tab shows how.".to_owned(),
                    ];
                    ask_first(
                        Act::RestorePoint,
                        None,
                        lines,
                        vec![Effect::RestorePoint { point }],
                    )
                }
                _ => Reaction::stays(Modal::Points { index }),
            }
        }
        _ => Reaction::stays(Modal::Points { index }),
    }
}

/// A restore point was put back (or part of it failed): say what happened, and how to undo it.
pub(crate) fn on_point_restored(
    state: &mut State,
    point: &str,
    result: Result<PointOutcome, String>,
) -> Vec<Effect> {
    match result {
        Err(why) => {
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
        Ok(outcome) if !outcome.failed.is_empty() => {
            state.notice = Some(Notice::Error(format!(
                "Put {} area(s) back to {point}, but {} failed: {}",
                outcome.restored,
                outcome.failed.len(),
                outcome.failed.join("; ")
            )));
            vec![Effect::LoadConfigs]
        }
        Ok(outcome) => {
            let undo = outcome
                .undo
                .map(|undo| format!(" Undo it: restore point {undo}"))
                .unwrap_or_default();
            state.after_reload = Some(Notice::Info(format!(
                "Put {} area(s) back to {point}.{undo}",
                outcome.restored
            )));
            vec![Effect::LoadConfigs]
        }
    }
}
