// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a person is told about an area: what could be protected, what protecting does, and why it failed.

use crate::protecting::coverage::{describe, Area, Info, Worth};
use crate::protecting::protect::SetupError;

/// One line for an area: its name, where it is, and whether it is protected.
pub fn area_line(area: &Area) -> String {
    let info = describe(&area.mount_point);
    let state = if area.is_protected() {
        "protected"
    } else {
        "NOT protected"
    };
    format!("{} ({}): {state}", info.title, area.mount_point)
}

/// What an area holds and whether it is worth protecting, for the list of areas.
pub fn area_detail(info: &Info) -> String {
    let verdict = match info.worth {
        Worth::Recommended => "Recommended.",
        Worth::Optional => "Up to you.",
        Worth::NotUseful => "Not worth protecting.",
    };
    format!(
        "It holds {}. {verdict} {}.",
        info.holds,
        capital(&info.reason)
    )
}

pub(super) fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// What protecting an area will do and will not do, for the person to read before they say yes.
pub fn protect_offer(mount_point: &str) -> String {
    let info = describe(mount_point);
    format!(
        "Protect {title} ({mount_point})?\n\n\
         It holds {holds}.\n\
         {verdict}: {reason}.\n\n\
         What snapall will do:\n  \
         - make a hidden folder called .snapshots inside {mount_point}, where its snapshots will be kept;\n  \
         - make sure snapshots of it are taken only when you ask (with snapall), never on a schedule, so nothing is taken or piles up on its own.\n\n\
         What it will not do: it does not copy, move or change any of your files, and it takes no snapshot yet.\n\n\
         Space: a snapshot takes almost nothing when it is taken and grows only as files change. How many are kept, and when the oldest are removed, are set for each area and shown in the Health tab.",
        title = info.title,
        holds = info.holds,
        reason = info.reason,
        verdict = match info.worth {
            Worth::Recommended => "Recommended",
            Worth::Optional => "Up to you",
            Worth::NotUseful => "Not recommended",
        },
    )
}

/// The offer for a window that is 24 lines tall: what it holds, whether it is worth it, what will and
/// will not happen. (How space and cleanup work is in the help and in `snapall setup`.)
pub fn protect_summary(mount_point: &str) -> String {
    let info = describe(mount_point);
    format!(
        "Protect {title} ({mount_point})?\n\n\
         It holds {holds}.\n\
         {verdict}: {reason}.\n\n\
         What snapall will do:\n  \
         - make a hidden folder called .snapshots inside {mount_point}, where its snapshots are kept;\n  \
         - make sure snapshots are taken only when you ask, never on a schedule.\n\n\
         It does not copy, move or change any of your files, and takes no snapshot yet.",
        title = info.title,
        holds = info.holds,
        reason = info.reason,
        verdict = match info.worth {
            Worth::Recommended => "Recommended",
            Worth::Optional => "Up to you",
            Worth::NotUseful => "Not recommended",
        },
    )
}

/// A failed attempt to protect an area, as a person should read it: what went wrong with what, and
/// whether anything was changed.
pub fn failure_text(title: &str, error: &SetupError) -> String {
    match error {
        SetupError::Failed(reason) => format!(
            "Could not protect {title}. The reason the system gave: {reason}. Nothing was changed."
        ),
        SetupError::LeftBehind { problem, area, .. } => format!(
            "Protecting {title} stopped half way ({problem}) and the part already done could not be taken back: it is left under the name `{area}`. Please report this at https://github.com/grenudi/snapall/issues."
        ),
        other => format!("Could not protect {title}: {other}. Nothing was changed."),
    }
}

/// What to say after an area was protected.
pub fn protected_text(mount_point: &str) -> String {
    let info = describe(mount_point);
    format!(
        "{} ({mount_point}) is protected now. No snapshot exists yet: the next time you take one (the snapall window, or `sudo snapall`), it will include this area.",
        info.title
    )
}
