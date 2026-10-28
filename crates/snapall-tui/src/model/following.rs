// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Following the changes that run on their own: looking at the journal once a second while anything runs,
//! and noticing when one that this window started has ended.

use super::*;

/// While anything runs, or this window started something that has not yet been seen to end, the journal
/// is read once a second, so that progress moves and the end is noticed even if another program caused it.
pub(crate) fn follow_what_runs(state: &mut State) -> Vec<Effect> {
    let running = state
        .activity
        .entries
        .iter()
        .any(|entry| entry.status == snapall_core::JournalStatus::Running);
    if !running && state.watching.is_empty() {
        state.poll_ticks = 0;
        return Vec::new();
    }
    state.poll_ticks += 1;
    if state.poll_ticks < 10 {
        return Vec::new();
    }
    state.poll_ticks = 0;
    vec![Effect::LoadHistory]
}

/// The journal was read: a change this window handed to the background may have ended, and then the lists
/// are looked at again, because it changed what they show. One that never appears in the journal is let go
/// of after a while, with a word about it.
pub(crate) fn what_this_window_started_has_ended(
    state: &mut State,
    entries: &[snapall_core::JournalEntry],
) -> Vec<Effect> {
    use snapall_core::JournalStatus;
    const LET_GO_AFTER: u32 = 30;
    let mut effects = Vec::new();
    let mut notice = None;
    state.watching.retain_mut(|watched| {
        match entries.iter().find(|entry| entry.id == watched.id) {
            None => {
                watched.unseen += 1;
                if watched.unseen > LET_GO_AFTER {
                    notice = Some(Notice::Error(
                        "A change started in the background never reported: look in Activity, and at the snapshots, to see whether it happened"
                            .into(),
                    ));
                    return false;
                }
                true
            }
            Some(entry) if entry.status == JournalStatus::Running => true,
            Some(entry) => {
                notice = Some(match entry.status {
                    JournalStatus::Done if entry.detail.is_empty() => {
                        Notice::Info(format!("{}: done", entry.what))
                    }
                    JournalStatus::Done => {
                        Notice::Info(format!("{}: done. {}", entry.what, entry.detail))
                    }
                    JournalStatus::Cancelled => {
                        Notice::Info(format!("{}: stopped on request. {}", entry.what, entry.detail))
                    }
                    _ => Notice::Error(format!("{}: {}", entry.what, entry.detail)),
                });
                effects = vec![Effect::LoadAreas, Effect::LoadHealth];
                false
            }
        }
    });
    if let Some(notice) = notice {
        // looking at the lists again would clear it: it is shown again when they have arrived
        if !effects.is_empty() {
            state.after_reload = Some(notice.clone());
        }
        state.notice = Some(notice);
    }
    effects
}
