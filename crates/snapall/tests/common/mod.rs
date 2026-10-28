// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared by the architecture and findability tests: the debts, written down as data, and the ratchet that may only lower them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// The debts, written down as data: for each rule, the offenders that exist today and how bad each is allowed to be.
/// Rust holds the rules and this file holds the debt, so paying one down is an edit to data and never to code.
fn read_debts() -> BTreeMap<String, BTreeMap<String, usize>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ratchets.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

static DEBTS_ARE_BEING_WRITTEN: Mutex<()> = Mutex::new(());

/// A ratchet. `observed` is what the code has now (name -> size); `limit` what is allowed without a reason; `list`
/// names the debts in `ratchets.json`: the offenders that are over the limit today, with their size.
///
/// A debt may only be paid. Run with `UPDATE_RATCHETS=1` and every debt that has shrunk, or gone, is lowered or removed in
/// the file; nothing is ever raised and nothing new is added, so growth and new offenders still fail.
pub fn ratchet(rule: &str, list: &str, observed: &BTreeMap<String, usize>, limit: usize) {
    let _writing = DEBTS_ARE_BEING_WRITTEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut all = read_debts();
    let debt = all.get(list).cloned().unwrap_or_default();
    let paying = std::env::var_os("UPDATE_RATCHETS").is_some();
    let mut problems = Vec::new();
    for (name, size) in observed {
        let most = debt.get(name).copied().unwrap_or(limit);
        if *size > most {
            problems.push(format!("`{name}` is {size} (the most allowed is {most})"));
        }
    }
    let mut remaining = BTreeMap::new();
    for (listed, most) in &debt {
        match observed.get(listed) {
            None if paying => {}
            None => problems.push(format!(
                "`{listed}` is on the debt list but no longer exists: take it off"
            )),
            Some(size) if *size <= limit && paying => {
                let _ = size;
            }
            Some(size) if *size <= limit => {
                problems.push(format!("`{listed}` is {size}, within the limit now: take it off the list, so it cannot grow back"));
            }
            Some(size) if size < most && paying => {
                remaining.insert(listed.clone(), *size);
            }
            Some(size) if size < most => {
                problems.push(format!("`{listed}` shrank to {size}: lower its entry from {most}, so it cannot grow back"));
            }
            Some(size) => {
                remaining.insert(listed.clone(), *size);
            }
        }
    }
    if paying && problems.is_empty() && remaining != debt {
        all.insert(list.to_owned(), remaining);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ratchets.json");
        std::fs::write(path, serde_json::to_string_pretty(&all).unwrap() + "\n").unwrap();
    }
    let over: Vec<String> = observed
        .iter()
        .filter(|(_, size)| **size > limit)
        .map(|(name, size)| format!("\"{name}\": {size},"))
        .collect();
    assert!(
        problems.is_empty(),
        "{rule}\n  {}\nover the limit of {limit} now:\n  {}\nto record paid debts: UPDATE_RATCHETS=1 cargo test -p snapall --test architecture",
        problems.join("\n  "),
        over.join("\n  ")
    );
}
