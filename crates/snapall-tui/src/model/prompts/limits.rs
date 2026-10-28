// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The editor of how many snapshots an area keeps: three settings, each adjusted with the arrows, and the
//! consequence of them shown as they are set. A change that would remove snapshots asks twice, because a
//! removed snapshot cannot be brought back.

use crate::model::*;
use snapall_core::keeping::limits::{LONGEST_PROTECTION_SECONDS, MOST_KEPT};
use snapall_core::{preview_limits, Act, Limits};

/// What a setting moves by: one, or ten with Page Up and Page Down. How young is in minutes.
const MINUTE: u64 = 60;

/// The machine's clock, as the same sort of seconds the snapshots' dates are.
pub(crate) fn now_seconds(state: &State) -> Option<i64> {
    let now = state.health.data.as_ref()?.facts.now.as_deref()?;
    snapall_core::vocabulary::calendar::seconds_from_snapshot_date(now)
}

/// Open the editor on an area, from what snapper has now.
pub(crate) fn open_limits(state: &mut State, config: AreaName) -> Vec<Effect> {
    let before = state
        .health
        .data
        .as_ref()
        .and_then(|data| data.facts.areas.iter().find(|area| area.config == config))
        .and_then(|area| area.retention.as_ref())
        .and_then(Limits::from_retention);
    let Some(before) = before else {
        state.notice = Some(Notice::Error(
            "snapper did not say how many this area keeps, so there is nothing to change".into(),
        ));
        return Vec::new();
    };
    state.modal = Some(Modal::Limits(LimitsEditor {
        area: config.clone(),
        before,
        limits: before,
        field: 0,
        snapshots: None,
        error: None,
    }));
    vec![Effect::LoadLimitsData { area: config }]
}

fn stays(editor: LimitsEditor) -> Reaction {
    Reaction::stays(Modal::Limits(editor))
}

/// Move the chosen setting by `steps` (negative to lower it).
fn adjust(editor: &mut LimitsEditor, steps: i64) {
    match editor.field {
        0 => {
            let kept = i64::from(editor.limits.keep) + steps;
            editor.limits.keep = u32::try_from(kept.clamp(1, i64::from(MOST_KEPT))).unwrap_or(1);
        }
        1 => {
            let seconds =
                i64::try_from(editor.limits.min_age_seconds).unwrap_or(0) + steps * MINUTE as i64;
            let longest = i64::try_from(LONGEST_PROTECTION_SECONDS).unwrap_or(i64::MAX);
            editor.limits.min_age_seconds = u64::try_from(seconds.clamp(0, longest)).unwrap_or(0);
        }
        _ => editor.limits.cleanup = !editor.limits.cleanup,
    }
    editor.error = None;
}

pub(crate) fn editing_limits(state: &mut State, key: Key, mut editor: LimitsEditor) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Up | Key::Char('k') => {
            editor.field = editor.field.saturating_sub(1);
            stays(editor)
        }
        Key::Down | Key::Char('j') | Key::Tab => {
            editor.field = (editor.field + 1).min(2);
            stays(editor)
        }
        Key::Left | Key::Char('-' | 'h') => {
            adjust(&mut editor, -1);
            stays(editor)
        }
        Key::Right | Key::Char('+' | '=' | 'l') => {
            adjust(&mut editor, 1);
            stays(editor)
        }
        Key::PageDown if editor.field < 2 => {
            adjust(&mut editor, -10);
            stays(editor)
        }
        Key::PageUp if editor.field < 2 => {
            adjust(&mut editor, 10);
            stays(editor)
        }
        Key::Char(' ') if editor.field == 2 => {
            adjust(&mut editor, 1);
            stays(editor)
        }
        Key::Enter => apply(state, editor),
        _ => stays(editor),
    }
}

fn apply(state: &mut State, mut editor: LimitsEditor) -> Reaction {
    if let Some(problem) = editor.limits.problem() {
        editor.error = Some(problem);
        return stays(editor);
    }
    let options = editor.limits.changes_from(&editor.before);
    if options.is_empty() {
        state.notice = Some(Notice::Info("Nothing was changed".into()));
        return Reaction::closes();
    }
    if !state.context.administrator {
        state.notice = Some(Notice::Info(
            "Changing what an area keeps needs administrator rights: start snapall with sudo."
                .into(),
        ));
        return stays(editor);
    }
    let would_remove = editor.snapshots.as_ref().map_or(0, |snapshots| {
        preview_limits(snapshots, &editor.limits, now_seconds(state)).would_remove
    });
    let removes = would_remove > 0;
    let mut lines = vec![format!(
        "Change what {} keeps: {}.",
        editor.area,
        editor.limits.describe()
    )];
    if let Some(snapshots) = &editor.snapshots {
        let preview = preview_limits(snapshots, &editor.limits, now_seconds(state));
        lines.push(snapall_core::limits_words(&preview, &editor.limits));
    }
    let then = vec![Effect::SetLimits {
        area: editor.area.clone(),
        options,
        before: editor.before,
    }];
    let Modal::Confirm(mut question) =
        confirm_modal(Act::ChangeLimits { removes }, None, lines, then)
    else {
        unreachable!("confirm_modal makes a confirmation")
    };
    // cancelled, it is back in the editor with the numbers as they were typed
    question.on_cancel = Some(Box::new(Modal::Limits(editor)));
    Reaction::stays(Modal::Confirm(question))
}
