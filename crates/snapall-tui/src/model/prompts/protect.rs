// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Protecting a part of the disk that has no snapshots yet: one offer at a time.

use crate::model::*;

/// The offer to protect a part of the disk: Tab and the arrows go to the next one, Enter protects the
/// one shown, Esc leaves. Without administrator rights, or with nothing to offer, it only informs.
pub(crate) fn protecting(state: &mut State, key: Key, index: usize) -> Reaction {
    let count = state.offers.len();
    let index = index.min(count.saturating_sub(1));
    let can_protect = count > 0 && state.context.administrator;
    match key {
        Key::Esc | Key::CtrlC | Key::Char('q') => Reaction::closes(),
        Key::Enter if !can_protect => Reaction::closes(),
        Key::Enter | Key::Char('y' | 'Y') if can_protect => {
            let offer = &state.offers[index];
            state.notice = Some(Notice::Info(format!(
                "protecting {} ({})",
                offer.title, offer.mount_point
            )));
            let effect = Effect::Protect {
                mount_point: offer.mount_point.clone(),
            };
            Reaction::closes_and_starts(vec![effect])
        }
        Key::Tab | Key::Right | Key::Down | Key::Char('j' | 'l') if count > 1 => {
            Reaction::stays(Modal::Protect {
                index: (index + 1) % count,
            })
        }
        Key::Left | Key::Up | Key::Char('k' | 'h') if count > 1 => {
            Reaction::stays(Modal::Protect {
                index: (index + count - 1) % count,
            })
        }
        _ => Reaction::stays(Modal::Protect { index }),
    }
}

pub(crate) fn on_protected(
    state: &mut State,
    mount_point: &str,
    result: Result<ConfigName, String>,
) -> Vec<Effect> {
    match result {
        Ok(_) => {
            state
                .offers
                .retain(|offer| offer.mount_point != mount_point);
            state.after_reload = Some(Notice::Info(snapall_core::explain::protected_text(
                mount_point,
            )));
            // the new area is in the list from now on, and takes part in "snapshot everything"
            vec![Effect::LoadConfigs]
        }
        Err(why) => {
            state.notice = Some(Notice::Error(why));
            Vec::new()
        }
    }
}
