// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restoring an area from a snapshot: showing what would change, asking, and what comes of it.

use crate::model::*;
use snapall_core::Act;

/// The restore prompt. Only a prompt with real changes to restore can be confirmed, and the system
/// root only when it is a single path of it. Anything else is information, and any key closes it.
pub(crate) fn confirming_restore(
    state: &mut State,
    key: Key,
    config: ConfigName,
    id: SnapshotId,
    path: Option<String>,
    stage: RestoreStage,
) -> Reaction {
    let actionable = matches!(
        &stage,
        RestoreStage::Ready(p) if (path.is_some() || !p.is_system_root) && !p.changes.is_empty()
    );
    // Every area of the snapshot's restore point, when it has one and any of it would change.
    if matches!(key, Key::Char('A')) {
        if let Some((c, i, offer)) = &state.point_offer {
            if *c == config && *i == id && offer.areas_to_change() > 0 {
                let (areas, point) = (offer.areas_to_change(), offer.point.clone());
                let lines = vec![
                    format!("Put {areas} area(s) back to restore point {}.", point.point),
                    "Each area's current state is saved first, so this can be rolled back: the Activity tab shows how.".to_owned(),
                ];
                return ask_first(
                    Act::RestorePoint,
                    None,
                    "Put every area back",
                    lines,
                    vec![Effect::RestorePoint { point }],
                );
            }
        }
    }
    match key {
        Key::Char('y' | 'Y') | Key::Enter if actionable => {
            let what = match &path {
                Some(path) => format!("Put {path} back to how snapshot #{id} had it."),
                None => format!("Put {config} back to how snapshot #{id} had it."),
            };
            let lines = vec![
                what,
                "How it is now is saved first, so this can be rolled back: the Activity tab shows how.".to_owned(),
            ];
            ask_first(
                Act::Restore,
                None,
                "Put files back",
                lines,
                vec![Effect::Restore { config, id, path }],
            )
        }
        Key::Esc | Key::CtrlC | Key::Char('n' | 'N') => Reaction::closes(),
        // information with nothing to confirm: any key closes it (but not while still looking)
        _ if !actionable && stage != RestoreStage::Loading => Reaction::closes(),
        _ => Reaction::stays(Modal::Restore {
            config,
            id,
            path,
            stage,
        }),
    }
}

pub(crate) fn open_restore(state: &mut State) -> Vec<Effect> {
    let Some(config) = state.selected_config().map(|c| c.name.clone()) else {
        return Vec::new();
    };
    if state.focus != Pane::Snapshots {
        state.notice = Some(Notice::Info(
            "move to the snapshots pane (right arrow) to choose the snapshot to restore".into(),
        ));
        return Vec::new();
    }
    let Some(id) = state.selected_snapshot().map(|s| s.id) else {
        state.notice = Some(Notice::Error("no snapshot to restore".into()));
        return Vec::new();
    };
    state.modal = Some(Modal::Restore {
        config: config.clone(),
        id,
        path: None,
        stage: RestoreStage::Loading,
    });
    state.point_offer = None;
    vec![
        Effect::LoadChanges {
            config: config.clone(),
            id,
            path: None,
        },
        // whether this snapshot is one of several taken together: the prompt offers them all
        Effect::LoadPointOffer { config, id },
    ]
}

pub(crate) fn on_changes_loaded(
    state: &mut State,
    config: &ConfigName,
    id: SnapshotId,
    result: Result<Preview, String>,
) -> Vec<Effect> {
    // The answer only matters to the prompt that asked. If it was closed meanwhile, drop it.
    if let Some(Modal::Restore {
        config: asked_config,
        id: asked_id,
        stage,
        ..
    }) = &mut state.modal
    {
        if asked_config == config && *asked_id == id && *stage == RestoreStage::Loading {
            *stage = match result {
                Ok(preview) => RestoreStage::Ready(preview),
                Err(error) => RestoreStage::Failed(error),
            };
        }
    }
    Vec::new()
}

pub(crate) fn on_restored(
    state: &mut State,
    config: &ConfigName,
    id: SnapshotId,
    path: Option<String>,
    result: Result<Restored, String>,
) -> Vec<Effect> {
    state.notice = Some(match (&result, &path) {
        (Ok(done), Some(path)) => Notice::Info(format!(
            "restored {path} from #{id}; the state before is #{}, restore it to undo",
            done.safety_snapshot
        )),
        (Ok(done), None) => Notice::Info(format!(
            "restored {config} to #{id}; the state before is #{}, restore it to undo",
            done.safety_snapshot
        )),
        (Err(error), _) => Notice::Error(error.clone()),
    });
    state.marked.clear();
    // Reload either way: a safety snapshot may exist even when the restore itself failed.
    match &state.file {
        Some(view) => vec![Effect::LoadFileVersions {
            config: view.target.config.clone(),
            path: view.target.path.clone(),
        }],
        None => vec![Effect::LoadSnapshots(config.clone())],
    }
}
