// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Groups of areas.

use serde_json::{json, Value};

/// Every group, `all` first: what it covers, and which of its areas are no longer protected.
pub fn groups(made: &snapall_core::Groups, protected: &[snapall_core::AreaName]) -> String {
    let every: Vec<String> = protected.iter().map(ToString::to_string).collect();
    let mut out = format!(
        "all: every protected area ({})
",
        every.join(", ")
    );
    for name in made.names() {
        let areas = made.get(name).unwrap_or(&[]);
        let list: Vec<String> = areas
            .iter()
            .map(|area| {
                if protected.contains(area) {
                    area.to_string()
                } else {
                    format!("{area} (no longer protected)")
                }
            })
            .collect();
        out.push_str(&format!(
            "{name}: {}
",
            list.join(", ")
        ));
    }
    if made.is_empty() {
        out.push_str(
            "
No groups of your own yet. Make one with: snapall groups add NAME AREA,AREA
",
        );
    }
    out
}

pub fn groups_json(made: &snapall_core::Groups, protected: &[snapall_core::AreaName]) -> Value {
    let mut all = vec![
        json!({ "name": "all", "areas": protected.iter().map(ToString::to_string).collect::<Vec<_>>(), "stored": false }),
    ];
    all.extend(made.names().map(|name| {
        let areas = made.get(name).unwrap_or(&[]);
        json!({
            "name": name.to_string(),
            "areas": areas.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "missing": areas.iter().filter(|a| !protected.contains(a)).map(ToString::to_string).collect::<Vec<_>>(),
            "stored": true,
        })
    }));
    json!(all)
}
