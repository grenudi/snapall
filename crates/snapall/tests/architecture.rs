// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fitness functions: the dependency rule, checked against `cargo metadata` on every test
//! run. Documentation alone does not keep boundaries; this does.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

fn workspace_packages() -> Vec<serde_json::Value> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "cargo metadata failed");
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    metadata["packages"].as_array().expect("packages").clone()
}

/// Normal (non-dev, non-build) dependencies of a workspace package.
fn normal_deps(name: &str) -> BTreeSet<String> {
    let packages = workspace_packages();
    let package = packages
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("package {name} not found"));
    package["dependencies"]
        .as_array()
        .expect("dependencies")
        .iter()
        .filter(|d| d["kind"].is_null())
        .map(|d| d["name"].as_str().expect("name").to_owned())
        .collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn core_depends_on_nothing_but_the_error_derive() {
    assert_eq!(normal_deps("snapall-core"), set(&["thiserror"]));
}

#[test]
fn the_snapper_adapter_depends_on_the_core_and_serialization_only() {
    assert!(normal_deps("snapall-snapper").is_subset(&set(&[
        "snapall-core",
        "serde",
        "serde_json"
    ])));
}

#[test]
fn the_observability_adapter_depends_on_the_core_and_serialization_only() {
    // It times operations and writes lines; it must not know any other adapter or any driver.
    let deps = normal_deps("snapall-observe");
    assert!(
        deps.is_subset(&set(&["snapall-core", "serde_json"])),
        "unexpected dependencies: {deps:?}"
    );
    assert!(deps.contains("snapall-core"));
}

#[test]
fn the_system_adapter_depends_on_the_core_and_serialization_only() {
    // It asks the machine questions the core states, and writes the journal as JSON lines.
    let deps = normal_deps("snapall-system");
    assert_eq!(
        deps,
        set(&["snapall-core", "serde_json"]),
        "unexpected dependencies: {deps:?}"
    );
}

#[test]
fn the_tui_depends_on_the_core_and_the_terminal_library_only() {
    let deps = normal_deps("snapall-tui");
    assert!(
        deps.is_subset(&set(&["snapall-core", "ratatui", "signal-hook"])),
        "unexpected dependencies: {deps:?}"
    );
    assert!(deps.contains("snapall-core"));
}

#[test]
fn drivers_and_adapters_never_know_each_other() {
    // The two driving adapters (CLI, TUI) and the driven adapter (snapper) meet only in the
    // composition root.
    for library in [
        "snapall-core",
        "snapall-snapper",
        "snapall-tui",
        "snapall-observe",
        "snapall-system",
    ] {
        let deps = normal_deps(library);
        for forbidden in [
            "snapall",
            "snapall-tui",
            "snapall-snapper",
            "snapall-observe",
            "snapall-system",
        ] {
            if forbidden != library {
                assert!(
                    !deps.contains(forbidden),
                    "{library} must not depend on {forbidden}"
                );
            }
        }
    }
}

#[test]
fn only_the_binary_may_know_argenv() {
    for library in [
        "snapall-core",
        "snapall-snapper",
        "snapall-tui",
        "snapall-observe",
        "snapall-system",
    ] {
        let deps = normal_deps(library);
        assert!(
            !deps.contains("argenv") && !deps.contains("argenv-resolve"),
            "{library} must not depend on argenv"
        );
    }
    assert!(normal_deps("snapall").contains("argenv-resolve"));
}

#[test]
fn nobody_depends_on_a_cli_parser_other_than_argenv() {
    for package in [
        "snapall-core",
        "snapall-snapper",
        "snapall-tui",
        "snapall-observe",
        "snapall-system",
        "snapall",
    ] {
        assert!(
            !normal_deps(package).contains("clap"),
            "{package} must not depend on clap: the invocation surface is declared with argenv"
        );
    }
}

/// The production code (comments and `#[cfg(test)]` modules removed) of every crate whose words a
/// person can read. The snapper adapter is left out: it is where the commands are actually run.
fn text_a_person_can_read() -> Vec<(String, String)> {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    for name in ["snapall", "snapall-core", "snapall-tui", "snapall-observe"] {
        let mut stack = vec![crates.join(name).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let source = std::fs::read_to_string(&path).unwrap();
                    let production = source.split("#[cfg(test)]").next().unwrap_or_default();
                    let code: String = production
                        .lines()
                        .filter(|line| !line.trim_start().starts_with("//"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    found.push((path.display().to_string(), code));
                }
            }
        }
    }
    found
}

/// A person is never told to run a raw `snapper` command: snapall does the setup, and where there is
/// something to run it is `snapall` itself. Found three times by hand before this existed.
#[test]
fn no_message_tells_a_person_to_run_a_raw_snapper_command() {
    for (file, code) in text_a_person_can_read() {
        for forbidden in ["snapper -c", "create-config", "set-config"] {
            assert!(
                !code.contains(forbidden),
                "`{forbidden}` in the production code of {file}: a person must never be sent outside snapall"
            );
        }
    }
}

/// The words snapall teaches are "protected area" and "snapshot". "snapper config" is the name of a
/// thing inside another program, and nobody reading snapall's output has been told what it is.
#[test]
fn no_message_calls_a_protected_area_a_snapper_area() {
    for (file, code) in text_a_person_can_read() {
        for forbidden in [
            "snapper config",
            "snapper's config",
            "snapper has no configs",
            "no configs",
        ] {
            assert!(
                !code.to_lowercase().contains(forbidden),
                "`{forbidden}` in the production code of {file}"
            );
        }
    }
}

/// A file that runs past a few hundred lines is doing more than one thing. The window was once two files
/// of 1,662 and 1,251 lines; each is now a module per concern, and this keeps it that way.
#[test]
fn no_file_of_the_window_grows_back_into_a_wall() {
    const LIMIT: usize = 450;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../snapall-tui/src");
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                let code = source.split("#[cfg(test)]").next().unwrap_or_default();
                assert!(
                    code.lines().count() <= LIMIT,
                    "{} has {} lines of code (the limit is {LIMIT}): split it by concern",
                    path.display(),
                    code.lines().count()
                );
            }
        }
    }
}

// ---- what the texts say about the command line is what the contract says -----------------------------

/// What the committed contract (which another test proves is the one the code produces) says exists.
struct Surface {
    commands: BTreeSet<String>,
    /// Long flags, with whether each takes a value.
    flags: BTreeMap<String, bool>,
    negatable: BTreeSet<String>,
}

fn the_contract() -> Surface {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../snapall.argenv.json");
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut surface = Surface {
        commands: BTreeSet::new(),
        flags: Default::default(),
        negatable: BTreeSet::new(),
    };
    for input in document["inputs"].as_array().unwrap() {
        for command in input["subcommands"].as_array().into_iter().flatten() {
            surface
                .commands
                .insert(command.as_str().unwrap().to_owned());
        }
        if let Some(long) = input["arg"]["long"].as_str() {
            surface.flags.insert(
                format!("--{long}"),
                input["arg"]["arity"].as_u64() == Some(1),
            );
            if input["arg"]["negatable"].as_bool() == Some(true) {
                surface.negatable.insert(format!("--{long}"));
            }
        }
    }
    // not an input of any command: the document itself
    surface.commands.insert("contract".to_owned());
    surface
}

/// The text of the README and the docs, which a person reads as much as the messages.
fn documents() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = vec![(
        "README.md".to_owned(),
        std::fs::read_to_string(root.join("README.md")).unwrap(),
    )];
    let mut stack = vec![root.join("docs")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                found.push((
                    path.display().to_string(),
                    std::fs::read_to_string(&path).unwrap(),
                ));
            }
        }
    }
    found
}

/// What is between a pair of backticks.
fn code_spans(text: &str) -> impl Iterator<Item = &str> {
    text.split('`').skip(1).step_by(2)
}

/// Written on purpose as a mistake, to show what snapall says to one.
const MISTAKES_ON_PURPOSE: [&str; 1] = ["snapall lst"];

/// A command or a flag that a text tells a person to type must exist. Found by hand, again and again, before
/// this was a test: a message that sends someone to `snapall frobnicate` is a bug nobody sees until they try it.
#[test]
fn every_command_and_flag_a_text_tells_a_person_to_type_exists_in_the_contract() {
    let surface = the_contract();
    let mut texts: Vec<(String, String)> = text_a_person_can_read();
    texts.extend(documents());
    let mut checked = 0;
    for (file, text) in &texts {
        for span in code_spans(text) {
            if !span.starts_with("snapall ") || MISTAKES_ON_PURPOSE.contains(&span.trim()) {
                continue;
            }
            // a table cell escapes a bar (`--yes\|--destroy`): either way it separates alternatives
            let spaced = span.replace("\\|", " ").replace('|', " ");
            let mut words = spaced.split_whitespace().skip(1);
            let mut named_a_command = false;
            while let Some(word) = words.next() {
                // `snapall "before theming"` is the shorthand for `create`: a quoted word is a description
                if word.starts_with('"') || word.starts_with('\'') {
                    continue;
                }
                let word = word.trim_matches(|c: char| ",.;:)\"[]|".contains(c));
                if let Some(flag) = word.strip_prefix("--") {
                    let name = format!("--{}", flag.split('=').next().unwrap_or(flag));
                    let known = surface.flags.get(&name).copied().or_else(|| {
                        // `--no-hints` is the other way of saying a flag that can be switched both ways
                        name.strip_prefix("--no-").and_then(|bare| {
                            let positive = format!("--{bare}");
                            surface.negatable.contains(&positive).then_some(false)
                        })
                    });
                    assert!(
                        known.is_some() || name == "--help" || name == "--version",
                        "`{name}` in `{span}` ({file}) is not a flag of snapall"
                    );
                    if known == Some(true) && !flag.contains('=') {
                        words.next(); // its value
                    }
                } else if word.starts_with('-')
                    || word.starts_with('<')
                    || word.chars().all(|c| !c.is_lowercase())
                {
                    // a short flag, a <PLACEHOLDER>, or an UPPER-CASE one
                } else if !named_a_command {
                    named_a_command = true;
                    assert!(
                        surface.commands.contains(word),
                        "`snapall {word}` in `{span}` ({file}) is not a command of snapall"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked > 100,
        "only {checked} mentions were checked: the scan has stopped finding them"
    );
}

/// And the other way: what snapall can do is written down for a person. A command that exists but is in no
/// text is one nobody finds.
#[test]
fn every_command_in_the_contract_is_in_the_readme() {
    let readme =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md"))
            .unwrap();
    let mentioned: BTreeSet<&str> = code_spans(&readme)
        .filter_map(|span| span.strip_prefix("snapall "))
        .flat_map(|rest| {
            rest.split_whitespace()
                .filter(|w| !w.starts_with('-'))
                .take(1)
        })
        .collect();
    for command in the_contract().commands {
        assert!(
            mentioned.contains(command.as_str()),
            "`snapall {command}` exists but the README never shows it: nobody can find what is not written down"
        );
    }
}

// ---- a handler reads like a sentence --------------------------------------------------------------------

/// The longest a command's handler may be. Past it, it is doing the work of several sentences (asking, doing,
/// recording, saying) and no longer reads as what the command is for.
const LONGEST_HANDLER: usize = 60;

/// Every top-level function of every file in `commands/`, with its text (not counting tests).
fn handlers() -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut folders = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/commands")];
    let mut files = Vec::new();
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            if entry.path().is_dir() {
                folders.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }
    for file in files {
        // the table of what can be asked for, and the command that does each, is the map to the handlers and not a
        // handler: one language, long because there is a lot to ask for
        if file.file_name().is_some_and(|name| name == "asked_for.rs") {
            continue;
        }
        let source = std::fs::read_to_string(file).unwrap();
        let lines: Vec<&str> = source
            .split("#[cfg(test)]")
            .next()
            .unwrap()
            .lines()
            .collect();
        let mut at = 0;
        while at < lines.len() {
            let name = ["pub fn ", "pub(super) fn ", "pub(crate) fn ", "fn "]
                .iter()
                .find_map(|prefix| lines[at].strip_prefix(prefix))
                .and_then(|rest| rest.split(['(', '<']).next());
            if let Some(name) = name {
                let end = (at..lines.len())
                    .find(|line| lines[*line] == "}")
                    .unwrap_or(at);
                found.push((name.to_owned(), lines[at..=end].join("\n")));
                at = end;
            }
            at += 1;
        }
    }
    found
}

/// Every handler and its length in lines.
fn handler_lengths() -> Vec<(String, usize)> {
    handlers()
        .into_iter()
        .map(|(name, text)| (name, text.lines().count()))
        .collect()
}

/// The code of a handler, without its comments and the text inside its quotation marks, which are not what it
/// does.
fn only_the_doing(text: &str) -> String {
    let mut out = String::new();
    for line in text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
    {
        let mut inside = false;
        for letter in line.chars() {
            if letter == '"' {
                inside = !inside;
            } else if !inside {
                out.push(letter);
            }
        }
        out.push('\n');
    }
    out
}

/// How many times a handler branches or repeats.
fn turns_in(text: &str) -> usize {
    let doing = only_the_doing(text);
    ["if ", "match ", "for ", "while ", "loop "]
        .iter()
        .map(|word| {
            doing
                .match_indices(word)
                .filter(|(at, _)| {
                    *at == 0 || !doing[..*at].ends_with(|c: char| c.is_alphanumeric() || c == '_')
                })
                .count()
        })
        .sum()
}

/// How many closures a handler spells out: iterator and `Option` plumbing, not what the command is for.
fn plumbing_in(text: &str) -> usize {
    let doing = only_the_doing(text);
    let letters: Vec<char> = doing.chars().collect();
    let mut found = 0;
    let mut at = 0;
    while at + 1 < letters.len() {
        // `|name|` or `|(a, b)|` or `|_|`, not `||` and not `a | b`
        if letters[at] == '|'
            && (letters[at + 1].is_alphabetic() || "_&(".contains(letters[at + 1]))
        {
            if let Some(close) = letters[at + 1..]
                .iter()
                .position(|c| *c == '|' || *c == '\n')
            {
                if letters[at + 1 + close] == '|' {
                    found += 1;
                    at += close + 1;
                }
            }
        }
        at += 1;
    }
    found
}

#[test]
fn a_command_handler_is_short_enough_to_read_as_a_sentence_and_the_debt_only_shrinks() {
    let lengths = handler_lengths();
    assert!(
        lengths.len() > 20,
        "the scan found {} handlers: it has stopped finding them",
        lengths.len()
    );
    let observed = lengths.into_iter().collect();
    ratchet(
        "Handlers too long to read as a sentence (split them by what they do):",
        "handlers_too_long",
        &observed,
        LONGEST_HANDLER,
    );
}

// ---- the shape of the code: every module a small language, every layer the top of those beneath ---------
//
// The rules (ADR 19, 20 and `docs/canon/small-languages.md`) are only kept if a machine keeps them: a person
// forgets, and so does an agent. Each rule below is a *ratchet*: the debt that exists today is written down
// with its size; nothing may get worse, nothing new may join, and an entry leaves the list the moment it is
// paid. Whoever fixes one is told to take it off, so the list can only shrink.

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
fn ratchet(rule: &str, list: &str, observed: &BTreeMap<String, usize>, limit: usize) {
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

/// Every Rust source file of every crate, as (path from the repository root, code without its tests).
fn all_sources() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned();
            if path.is_dir() {
                if name != "tests" && name != "target" {
                    stack.push(path);
                }
            } else if name.ends_with(".rs") && path.components().any(|c| c.as_os_str() == "src") {
                let text = std::fs::read_to_string(&path).unwrap();
                if text.contains("#![cfg(test)]") {
                    continue; // only compiled for tests: not production code
                }
                let relative = path
                    .strip_prefix(&root)
                    .unwrap()
                    .display()
                    .to_string()
                    .replace("../", "");
                found.push((
                    relative,
                    text.split("#[cfg(test)]").next().unwrap().to_owned(),
                ));
            }
        }
    }
    found.sort();
    found
}

fn lines_of(code: &str) -> usize {
    code.lines().count()
}

/// How long a file may be before it is more than one thing.
const LONGEST_FILE: usize = 300;
/// How long an entry file (`lib.rs`, `main.rs`, `mod.rs`) may be before a level is probably hiding in it: its
/// job is to say what the module is for and offer its vocabulary. An alarm that finds candidates, not the
/// definition of a good module.
const LONGEST_FRONT_FILE: usize = 120;

#[test]
fn a_file_is_one_thing_short_enough_to_read_whole_and_the_debt_only_shrinks() {
    let observed = all_sources()
        .into_iter()
        .map(|(path, code)| (path, lines_of(&code)))
        .collect();
    ratchet(
        "Files over the length of one thing:",
        "files_too_long",
        &observed,
        LONGEST_FILE,
    );
}

#[test]
fn an_entry_file_longer_than_a_screen_hides_a_level_and_the_debt_only_shrinks() {
    let observed = all_sources()
        .into_iter()
        .filter(|(path, _)| {
            ["lib.rs", "main.rs", "mod.rs"]
                .iter()
                .any(|front| path.ends_with(front))
        })
        .map(|(path, code)| (path, lines_of(&code)))
        .collect();
    ratchet(
        "Entry files so long that a level is probably hiding in them:",
        "entry_files_too_long",
        &observed,
        LONGEST_FRONT_FILE,
    );
}

/// Text for a person is *returned* by a command and shown by the one that presents it, so that everything a
/// person can be shown is in one place. Only the asker (which has to put a question and read the answer)
/// prints on its own.
#[test]
fn nothing_prints_to_a_person_except_what_presents_and_what_asks() {
    let mut observed = std::collections::BTreeMap::new();
    for (path, code) in all_sources() {
        if path.ends_with("ask.rs") || path.contains("snapall-tui") {
            continue;
        }
        let prints = code
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .map(|line| {
                ["println!(", "print!("] /* eprintln!( and eprint!( contain these: each print is counted once */
                    .iter()
                    .map(|p| line.matches(p).count())
                    .sum::<usize>()
            })
            .sum::<usize>();
        if prints > 0 {
            observed.insert(path, prints);
        }
    }
    ratchet(
        "Files that print by themselves:",
        "prints_outside_the_presenter",
        &observed,
        0,
    );
}

/// Above the adapters the words are the person's (`area`, `snapshot`, `restore point`), never the tools':
/// snapper's `config`, `subvolume`, Btrfs and systemd belong to the code that wraps them and nowhere above it.
#[test]
fn above_the_adapters_the_names_are_the_persons_and_not_the_tools() {
    let foreign = ["snapper", "btrfs", "subvol", "systemctl", "config"];
    let mut observed = std::collections::BTreeMap::new();
    for (path, code) in all_sources() {
        let crate_name = path.split('/').nth(1).unwrap_or_default().to_owned();
        if !["snapall-core", "snapall-tui", "snapall"].contains(&crate_name.as_str()) {
            continue; // the adapters are where the tools' words belong
        }
        for line in code
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
        {
            for keyword in [
                "fn ", "struct ", "enum ", "const ", "let ", "mod ", "type ", "static ",
            ] {
                let mut rest = line;
                while let Some(at) = rest.find(keyword) {
                    let after = &rest[at + keyword.len()..];
                    let name: String = after
                        .trim_start_matches("mut ")
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if foreign
                        .iter()
                        .any(|word| name.to_lowercase().contains(word))
                    {
                        *observed.entry(crate_name.clone()).or_insert(0) += 1;
                    }
                    rest = after;
                }
            }
        }
    }
    ratchet(
        "Crates with names in the tools' words above the adapters:",
        "tool_words_in_names",
        &observed,
        0,
    );
}

// ---- the outline: the code, read as a table of contents ----------------------------------------------------

/// What a module says it is for: the first sentence of its `//!`. A module that says nothing has no place in
/// the outline, and so none in the code.
fn purpose_of(path: &str, code: &str) -> String {
    let doc: Vec<&str> = code
        .lines()
        .skip_while(|line| !line.starts_with("//!"))
        .take_while(|line| line.starts_with("//!") && !line[3..].trim().is_empty())
        .map(|line| line[3..].trim())
        .collect();
    assert!(
        !doc.is_empty(),
        "`{path}` does not say what it is for: its first doc line (`//!`) is missing"
    );
    let paragraph = doc.join(" ");
    // the first sentence: up to a full stop that ends a word, not one inside `e.g.`
    let end = paragraph
        .match_indices(". ")
        .map(|(at, _)| at + 1)
        .find(|at| !paragraph[..*at].ends_with("e.g.") && !paragraph[..*at].ends_with("i.e."))
        .unwrap_or(paragraph.len());
    paragraph[..end].to_owned()
}

/// The outline, generated: every crate, and under it every module with what it says it is for. Written by no
/// one, so it is always what the code says.
fn outline() -> String {
    let mut out = String::from(
        "<!--\nSPDX-FileCopyrightText: 2026 grenudi\n\nSPDX-License-Identifier: GPL-3.0-or-later\n-->\n\n\
         # Outline\n\nWhat each part of snapall says it is for, read from the first line of each module. \
         Generated: do not edit. After changing what a module is for, run\n\
         `UPDATE_OUTLINE=1 cargo test -p snapall --test architecture the_outline`.\n",
    );
    let mut current = String::new();
    for (path, code) in all_sources() {
        let mut parts = path.splitn(4, '/');
        let (_, crate_name, _, inside) = (
            parts.next(),
            parts.next().unwrap_or_default(),
            parts.next(),
            parts.next().unwrap_or_default(),
        );
        let purpose = purpose_of(&path, &code);
        if crate_name != current {
            current = crate_name.to_owned();
            out.push_str(&format!("\n## {crate_name}\n\n"));
        }
        let depth = inside.matches('/').count();
        out.push_str(&format!("{}- `{inside}`: {purpose}\n", "  ".repeat(depth)));
    }
    out
}

#[test]
fn the_outline_of_the_code_is_generated_from_what_each_module_says_it_is_for() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../OUTLINE.md");
    let outline = outline();
    if std::env::var_os("UPDATE_OUTLINE").is_some() {
        std::fs::write(&path, &outline).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == outline,
        "OUTLINE.md is not what the code says. After changing what a module is for, run:\n  UPDATE_OUTLINE=1 cargo test -p snapall --test architecture the_outline"
    );
}

// ---- a story is a sequence of verbs: it branches little and spells out little plumbing --------------------

/// A handler decides *which* and *in what order* and leaves *how* to the layer below: it may turn (branch or
/// repeat) this often.
const MOST_TURNS_IN_A_STORY: usize = 4;
/// And it may spell out this many closures: more is iterator and `Option` plumbing that a named verb would hide.
const MOST_PLUMBING_IN_A_STORY: usize = 2;

#[test]
fn a_story_branches_little_and_the_debt_only_shrinks() {
    let observed = handlers()
        .into_iter()
        .map(|(name, text)| (name, turns_in(&text)))
        .collect();
    ratchet(
        "Handlers that turn more often than a story should:",
        "handlers_that_branch_too_much",
        &observed,
        MOST_TURNS_IN_A_STORY,
    );
}

#[test]
fn a_story_spells_out_little_plumbing_and_the_debt_only_shrinks() {
    let observed = handlers()
        .into_iter()
        .map(|(name, text)| (name, plumbing_in(&text)))
        .collect();
    ratchet(
        "Handlers that spell out plumbing a verb should hide:",
        "handlers_with_too_much_plumbing",
        &observed,
        MOST_PLUMBING_IN_A_STORY,
    );
}

// ---- a change cannot be made unasked: the type says so, and the one exception is counted -----------------------

/// `Change::begin` takes the leave that asking gives, so a change cannot begin without having asked. `setup` asks
/// area by area as it works and so cannot show one leave first: it is the one named exception, and this keeps it one.
#[test]
fn only_one_change_begins_without_a_leave_and_it_is_the_one_that_asks_as_it_goes() {
    let uses: Vec<String> = all_sources()
        .into_iter()
        .filter(|(_, code)| code.contains("Change::begin_asking_along("))
        .map(|(path, code)| {
            format!(
                "{path} x{}",
                code.matches("Change::begin_asking_along(").count()
            )
        })
        .collect();
    assert_eq!(
        uses,
        ["crates/snapall/src/commands/protecting.rs x1"],
        "a change that begins without the leave of asking is a change that can be made unasked"
    );
}

// ---- the canon text's worked example is the source, not a copy of it ---------------------------------------

/// The canon text shows the real `delete` story. It is only ever true if it is the source, so it must equal it: the
/// moment the code changes, this says the text no longer does.
#[test]
fn the_worked_example_in_the_canon_is_the_code_it_shows() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        std::fs::read_to_string(root.join("crates/snapall/src/commands/deleting.rs")).unwrap();
    let start = source.find("pub fn delete(").unwrap();
    let story = source[start..].split("\n}\n").next().unwrap().to_owned() + "\n}";
    let canon = std::fs::read_to_string(root.join("docs/canon/small-languages.md")).unwrap();
    assert!(
        canon.contains(&story),
        "docs/canon/small-languages.md shows a `delete` that is not the one in deleting.rs: copy the function again"
    );
}

// ---- the documents name only what is there -------------------------------------------------------------------

/// A document that is written in the present tense must point at what exists: a path that moved is a sentence that
/// became false. ADRs are exempt: they are dated records of a decision and are never rewritten to match the present.
#[test]
fn the_documents_name_only_files_and_folders_that_exist() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut documents = vec![
        "README.md",
        "ARCHITECTURE.md",
        "AGENTS.md",
        "CONTRIBUTING.md",
    ]
    .into_iter()
    .map(|name| root.join(name))
    .collect::<Vec<_>>();
    for entry in std::fs::read_dir(root.join("docs")).unwrap().flatten() {
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "md")
        {
            documents.push(entry.path());
        }
    }
    let everywhere: Vec<PathBuf> = {
        let mut found = Vec::new();
        let mut stack = vec![
            root.join("crates"),
            root.join("packaging"),
            root.join("dev"),
        ];
        while let Some(folder) = stack.pop() {
            for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    if entry.file_name() != "target" {
                        stack.push(entry.path());
                    }
                } else {
                    found.push(entry.path());
                }
            }
        }
        found
    };
    let mut missing = Vec::new();
    for document in documents.iter().filter(|path| path.exists()) {
        let text = std::fs::read_to_string(document).unwrap();
        for line in text.lines() {
            for quoted in line.split('`').skip(1).step_by(2) {
                let looks_like_a_path = quoted.ends_with('/')
                    || [".rs", ".toml", ".sh", ".py", ".json"]
                        .iter()
                        .any(|extension| quoted.ends_with(extension));
                // only what is in this repository: not an extension, a system path, a URL or an option
                if !looks_like_a_path
                    || quoted.contains(' ')
                    || quoted.contains('*')
                    || quoted.contains("://")
                    || quoted.starts_with('-')
                    || quoted.starts_with('/')
                    || (quoted.starts_with('.') && !quoted.starts_with("./"))
                {
                    continue;
                }
                let name = quoted.trim_start_matches("./");
                let given = root.join(name).exists() || root.join("crates").join(name).exists();
                let bare = !name.contains('/')
                    && everywhere
                        .iter()
                        .any(|path| path.file_name().is_some_and(|file| file == name));
                let inside = name.contains('/')
                    && everywhere
                        .iter()
                        .any(|path| path.to_string_lossy().contains(name.trim_end_matches('/')));
                if !given && !bare && !inside {
                    missing.push(format!(
                        "{}: `{quoted}`",
                        document.strip_prefix(&root).unwrap().display()
                    ));
                }
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "documents name what is not there:\n  {}",
        missing.join("\n  ")
    );
}
