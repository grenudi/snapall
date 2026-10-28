// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window is told it could protect, and the sentence that says so.

use super::*;
use snapall_core::protecting::coverage::{self, Worth};
use snapall_core::vocabulary::explain::{self};
use snapall_core::SnapshotBackend;

/// The parts of the disk the window offers to protect: the recommended ones first, then the ones that
/// are up to the person. What is not worth protecting is not offered.
pub fn offers(backend: &dyn SnapshotBackend, facts: &Facts) -> Vec<snapall_tui::Offer> {
    let Ok(areas) = find_the_areas(backend, facts) else {
        return Vec::new();
    };
    let mut offers: Vec<snapall_tui::Offer> = areas
        .iter()
        .filter(|area| !area.is_protected())
        .filter_map(|area| {
            let info = coverage::describe(&area.mount_point);
            (info.worth != Worth::NotUseful).then(|| snapall_tui::Offer {
                mount_point: area.mount_point.clone(),
                title: info.title,
                summary: explain::protect_summary(&area.mount_point),
                recommended: info.worth == Worth::Recommended,
            })
        })
        .collect();
    offers.sort_by_key(|offer| !offer.recommended); // stable: the recommended first, in disk order
    offers
}

/// The footer line for when something recommended has no snapshots: what it is and what to press.
pub fn hint_for(offers: &[snapall_tui::Offer]) -> Option<String> {
    let titles: Vec<&str> = offers
        .iter()
        .filter(|offer| offer.recommended)
        .map(|offer| offer.title.as_str())
        .collect();
    if titles.is_empty() {
        return None;
    }
    let verb = if titles.len() == 1 { "has" } else { "have" };
    Some(format!(
        "{} {verb} no snapshots yet. Press p to see what that means and protect {}.",
        titles.join(" and "),
        if titles.len() == 1 { "it" } else { "them" }
    ))
}
