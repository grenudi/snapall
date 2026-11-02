// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the file history screen does when the versions of its file arrive.

use crate::model::*;
use snapall_core::{AreaName, FileVersion};

pub(crate) fn on_file_versions_loaded(
    state: &mut State,
    area: &AreaName,
    path: &str,
    result: Result<Vec<FileVersion>, String>,
) -> Vec<Effect> {
    if let Some(view) = state.file.as_mut() {
        if &view.target.area == area && view.target.path == path {
            view.versions = match result {
                Ok(list) => Load::Loaded(list),
                Err(error) => Load::Failed(error),
            };
            let len = match &view.versions {
                Load::Loaded(list) => list.len(),
                _ => 0,
            };
            view.cursor = view.cursor.min(len.saturating_sub(1));
        }
    }
    Vec::new()
}
