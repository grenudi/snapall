// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The cold read, as a test: does the generated outline alone lead a stranger to the right module?
//!
//! The goal of the whole structure is that a person can learn what snapall does, and where, by reading names and one
//! sentence for each module. Size limits only measure proxies for that. This asks the real question. Each question in
//! `findability.json` is written the way someone new to the code would ask it. Every module is scored by how well its
//! path and its purpose sentence match the question (a small deterministic ranking, no model and no network), and the
//! module that really answers it must be among the first few. A question that fails shows a name or a sentence that
//! does not say what its module is for.
//!
//! To see the ranking for each question: `FINDABILITY_REPORT=1 cargo test -p snapall --test findability -- --nocapture`.
//! To add a question, add it to the JSON with the module (or modules) that answer it: it is a regression test from then on.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

mod common;
use common::ratchet;

/// Words that carry no meaning about a program.
const STOP: &[&str] = &[
    "a", "an", "the", "of", "to", "in", "is", "it", "and", "or", "for", "on", "with", "what",
    "where", "how", "does", "do", "when", "which", "are", "be", "by", "as", "at", "from", "that",
    "this", "can", "i", "we", "you", "one", "its", "if", "then", "not", "no", "who", "get", "gets",
    "there", "their", "them", "so", "than", "into", "about", "after", "before", "per", "while",
    "code", "program", "snapall",
];

/// Forms that no suffix rule joins to their verb.
const IRREGULAR: &[(&str, &str)] = &[
    ("taken", "take"),
    ("took", "take"),
    ("kept", "keep"),
    ("made", "make"),
    ("gone", "go"),
    ("ran", "run"),
    ("written", "write"),
    ("shown", "show"),
    ("held", "hold"),
    ("done", "do"),
];

fn stem(word: &str) -> String {
    let word = IRREGULAR
        .iter()
        .find(|(form, _)| *form == word)
        .map_or(word, |(_, verb)| *verb);
    let mut stem = word.to_owned();
    for suffix in ["ing", "ed", "es", "er", "s", "ly"] {
        if word.len() > suffix.len() + 2 && word.ends_with(suffix) {
            stem = word[..word.len() - suffix.len()].to_owned();
            break;
        }
    }
    // `delete` and `deleting` meet at `delet`, `take` and `taking` at `tak`
    if stem.len() > 3 && stem.ends_with('e') {
        stem.pop();
    }
    stem
}

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOP.contains(w) && !["rs", "mod", "md"].contains(w))
        .map(stem)
        .collect()
}

/// Every module in the outline: `crate/path` and the words of its path and of its purpose.
fn modules() -> Vec<(String, BTreeSet<String>, BTreeSet<String>)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let outline = std::fs::read_to_string(root.join("OUTLINE.md")).unwrap();
    let (mut found, mut current) = (Vec::new(), String::new());
    for line in outline.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            current = name.trim().to_owned();
        } else if let Some(entry) = line.trim_start().strip_prefix("- `") {
            let (path, purpose) = entry
                .split_once("`: ")
                .unwrap_or((entry.trim_end_matches('`'), ""));
            let name = format!("{current}/{path}");
            found.push((
                name,
                words(&path.replace(['/', '_', '.'], " "))
                    .into_iter()
                    .collect(),
                words(purpose).into_iter().collect(),
            ));
        }
    }
    found
}

#[test]
fn the_outline_alone_leads_a_stranger_to_the_module_that_answers_each_question() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/findability.json");
    let data: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
    let top = data["top"].as_u64().unwrap() as usize;
    let modules = modules();
    assert!(
        modules.len() > 100,
        "the outline has {} modules: it has stopped being read",
        modules.len()
    );
    // a word that names only a few modules says more than one that names many
    let mut spread: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, path, purpose) in &modules {
        for word in path.union(purpose) {
            *spread.entry(word.as_str()).or_default() += 1;
        }
    }
    let idf = |word: &str| {
        (((modules.len() + 1) as f64) / ((spread.get(word).copied().unwrap_or(0) + 1) as f64)).ln()
            + 1.0
    };
    let report = std::env::var_os("FINDABILITY_REPORT").is_some();
    let mut gaps: BTreeMap<String, usize> = BTreeMap::new();
    let mut ranks = Vec::new();
    for item in data["questions"].as_array().unwrap() {
        let question = item["question"].as_str().unwrap();
        let answers: Vec<&str> = item["answers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        for answer in &answers {
            assert!(
                modules.iter().any(|(name, _, _)| name == answer),
                "`{answer}` (the answer to \"{question}\") is not in the outline"
            );
        }
        let wanted: BTreeSet<String> = words(question).into_iter().collect();
        let mut scored: Vec<(f64, &str)> = modules
            .iter()
            .map(|(name, path, purpose)| {
                let score: f64 = wanted
                    .iter()
                    .map(|w| {
                        if path.contains(w) {
                            2.0 * idf(w)
                        } else if purpose.contains(w) {
                            idf(w)
                        } else {
                            0.0
                        }
                    })
                    .sum();
                (score, name.as_str())
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(b.1)));
        let rank = scored
            .iter()
            .position(|(_, name)| answers.contains(name))
            .map_or(usize::MAX, |at| at + 1);
        ranks.push(rank);
        if report {
            let best: Vec<&str> = scored.iter().take(3).map(|(_, n)| *n).collect();
            println!(
                "{:>3}  {question}\n       found: {}\n       wanted: {}",
                if rank == usize::MAX {
                    "-".into()
                } else {
                    rank.to_string()
                },
                best.join(", "),
                answers.join(" | ")
            );
        }
        gaps.insert(question.to_owned(), rank.min(9999));
    }
    let found_first = ranks.iter().filter(|r| **r == 1).count();
    println!("findability: {found_first} of {} questions put the answer first; {} within the first {top}", ranks.len(), ranks.iter().filter(|r| **r <= top).count());
    ratchet(
        "Questions the outline does not answer within the first few modules (name the module for what it is for, in the words a stranger would use):",
        "findability_gaps",
        &gaps,
        top,
    );
}
