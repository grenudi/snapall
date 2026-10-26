// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The prompt that asks before deleting snapshots.

use crate::view::*;

pub(crate) fn delete_spec(config: &ConfigName, ids: &[SnapshotId]) -> ModalSpec {
    let list: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
    let lines = vec![
        Line::from(format!("Delete {} snapshot(s) of {config}?", ids.len())),
        Line::styled(list.join(", "), Style::new().bold()),
        Line::from(""),
        Line::from("Your files are not touched: only the saved moments are removed, and the space they held is freed."),
        Line::styled("This cannot be undone.", Style::new().fg(Color::Yellow)),
    ];
    ModalSpec {
        title: " Delete ".into(),
        lines,
        color: Color::Red,
        width: 56,
        height: 9,
        buttons: vec![confirm_button("Delete"), cancel_button()],
    }
}
