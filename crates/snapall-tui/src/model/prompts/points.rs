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
            Some(Modal::Points {
                index,
                points: Some(points),
                ..
            }) => points.get(*index),
            _ => None,
        }
    }
}

/// The restore points arrived: show them, and look up what the first would do. If the screen was closed meanwhile
/// there is nowhere for them to go, and they are dropped.
pub(crate) fn on_points_loaded(
    state: &mut State,
    result: Result<Vec<snapall_core::RestorePoint>, String>,
) -> Vec<Effect> {
    let Some(Modal::Points { points: slot, .. }) = &mut state.modal else {
        return Vec::new();
    };
    match result {
        Err(why) => {
            state.modal = None;
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
        Ok(points) => {
            let first = points.first().cloned();
            *slot = Some(points);
            first
                .map(|point| vec![Effect::LoadPointPreview { point }])
                .unwrap_or_default()
        }
    }
}

/// The restore points screen: Up and Down choose, Enter or `A` puts every area of the chosen one back,
/// Esc leaves. Nothing happens to a point whose areas already match.
pub(crate) fn browsing_points(
    key: Key,
    index: usize,
    points: Option<Vec<snapall_core::RestorePoint>>,
    preview: Option<PointOffer>,
) -> Reaction {
    let count = points.as_ref().map_or(0, Vec::len);
    let index = index.min(count.saturating_sub(1));
    let moved = |to: usize, points: Option<Vec<snapall_core::RestorePoint>>| {
        let effects = points
            .as_ref()
            .and_then(|points| points.get(to))
            .map(|point| {
                vec![Effect::LoadPointPreview {
                    point: point.clone(),
                }]
            })
            .unwrap_or_default();
        Reaction::stays_and_starts(
            Modal::Points {
                index: to,
                points,
                preview: None,
            },
            effects,
        )
    };
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Down | Key::Char('j') | Key::Tab if index + 1 < count => moved(index + 1, points),
        Key::Up | Key::Char('k') if index > 0 => moved(index - 1, points),
        Key::Enter | Key::Char('A') => {
            let chosen = points
                .as_ref()
                .and_then(|points| points.get(index))
                .cloned();
            match (chosen, &preview) {
                (Some(point), Some(shown))
                    if shown.point.point == point.point && shown.areas_to_change() > 0 =>
                {
                    let lines = vec![
                        format!(
                            "Put {} area(s) back to restore point {}.",
                            shown.areas_to_change(),
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
                _ => Reaction::stays(Modal::Points {
                    index,
                    points,
                    preview,
                }),
            }
        }
        _ => Reaction::stays(Modal::Points {
            index,
            points,
            preview,
        }),
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
            vec![Effect::LoadAreas]
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
            vec![Effect::LoadAreas]
        }
    }
}
