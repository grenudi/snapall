// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stopping the protection of an area: what it would do, in plain words, and the area's name typed out
//! before it does it. The one prompt where Enter alone, or a `y`, is never enough.

use crate::model::*;
use snapall_core::Weight;

/// Open the prompt for an area, and ask for the plan of what stopping would do.
pub(crate) fn open_unprotect(state: &mut State, area: AreaName) -> Vec<Effect> {
    state.modal = Some(Modal::StopProtecting(StopProtecting {
        area: area.clone(),
        plan: None,
        typed: String::new(),
        error: None,
    }));
    vec![Effect::LoadUnprotectPlan { area }]
}

fn stays(prompt: StopProtecting) -> Reaction {
    Reaction::stays(Modal::StopProtecting(prompt))
}

pub(crate) fn confirming_unprotect(
    state: &mut State,
    key: Key,
    mut prompt: StopProtecting,
) -> Reaction {
    match key {
        Key::Esc | Key::CtrlC => Reaction::closes(),
        // a plan that says no (the system area, say) leaves nothing to type
        Key::Enter if matches!(prompt.plan, Some(Err(_))) => Reaction::closes(),
        Key::Backspace => {
            prompt.typed.pop();
            prompt.error = None;
            stays(prompt)
        }
        Key::Char(c) if !c.is_control() && matches!(prompt.plan, Some(Ok(_))) => {
            if prompt.typed.chars().count() < 64 {
                prompt.typed.push(c);
            }
            prompt.error = None;
            stays(prompt)
        }
        Key::Enter if prompt.plan.is_none() => stays(prompt), // still looking
        Key::Enter => {
            if !Weight::Destroys.accepts(&prompt.typed, Some(prompt.area.as_str())) {
                prompt.error = Some(Weight::Destroys.instruction(Some(prompt.area.as_str())));
                return stays(prompt);
            }
            if !state.context.administrator {
                state.notice = Some(Notice::Info(
                    "Stopping protection needs administrator rights: start snapall with sudo."
                        .into(),
                ));
                return stays(prompt);
            }
            state.notice = Some(Notice::Info(format!(
                "stopping protection of {}",
                prompt.area
            )));
            Reaction::closes_and_starts(vec![Effect::StopProtecting { area: prompt.area }])
        }
        _ => stays(prompt),
    }
}
