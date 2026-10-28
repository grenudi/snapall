// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the command line says, as text and as JSON.

#![cfg(test)]

use crate::contract::{Command, Contract, LogFormat};
use argenv_resolve::{lint, Finding, Invocation, LintPolicy, Severity};
use snapall_core::{AreaName, Description, SnapshotId};
use std::path::PathBuf;

use std::collections::BTreeMap;

use super::parsing::edit_distance;
use super::*;

fn run(args: &[&str], env: &[(&str, &str)]) -> Result<Parsed, UsageError> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let env: BTreeMap<String, String> = env
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    parse(&args, &env)
}

fn ok(args: &[&str]) -> Parsed {
    run(args, &[]).unwrap_or_else(|e| panic!("{args:?} should parse: {e}"))
}

fn err(args: &[&str]) -> String {
    run(args, &[])
        .expect_err(&format!("{args:?} should be rejected"))
        .to_string()
}

fn names(list: &[&str]) -> Vec<AreaName> {
    list.iter().map(|n| AreaName::parse(n).unwrap()).collect()
}

fn create(only: Option<&[&str]>, description: &str, dry_run: bool) -> Request {
    Request::Create {
        only: only.map(names),
        description: Description::parse(description).unwrap(),
        dry_run,
        job: None,
        group: None,
    }
}

// ---- create, and its shorthand ------------------------------------------------

#[test]
fn no_arguments_means_create_with_the_default_description() {
    assert_eq!(ok(&[]).request, create(None, "snapall", false));
}

#[test]
fn a_bare_description_is_shorthand_for_create() {
    assert_eq!(
        ok(&["before theming"]).request,
        create(None, "before theming", false)
    );
    assert_eq!(
        ok(&["create", "before theming"]).request,
        create(None, "before theming", false)
    );
}

#[test]
fn flags_may_surround_the_shorthand_description() {
    assert_eq!(
        ok(&["--dry-run", "nightly", "--only", "home,root"]).request,
        create(Some(&["home", "root"]), "nightly", true)
    );
}

#[test]
fn a_description_after_double_dash_may_look_like_a_flag() {
    assert_eq!(
        ok(&["--", "--weird"]).request,
        create(None, "--weird", false)
    );
}

#[test]
fn a_one_word_typo_of_a_command_is_refused_not_snapshotted() {
    let message = err(&["lst"]);
    assert!(message.contains("did you mean `list`"), "{message}");
    assert!(message.contains("snapall create lst"), "{message}");
    // ...and the escape hatch works.
    assert_eq!(ok(&["create", "lst"]).request, create(None, "lst", false));
    // A real one-word description is not a typo of anything.
    assert_eq!(
        ok(&["before-theming"]).request,
        create(None, "before-theming", false)
    );
}

#[test]
fn bad_descriptions_and_area_names_are_rejected_at_the_boundary() {
    assert!(err(&["create", "   "]).contains("description"));
    assert!(err(&["create", "--only", "root,"]).contains("--only"));
    assert!(err(&["create", "--only", ""]).contains("--only"));
    assert!(err(&["create", "--only", "a b"]).contains("not allowed"));
}

// ---- environment: one setting, two doors --------------------------------------

#[test]
fn the_environment_supplies_values_and_arguments_win() {
    let from_env = run(&[], &[("SNAPALL_ONLY", "home")]).unwrap();
    assert_eq!(from_env.request, create(Some(&["home"]), "snapall", false));

    let both = run(&["--only", "root"], &[("SNAPALL_ONLY", "home")]).unwrap();
    assert_eq!(both.request, create(Some(&["root"]), "snapall", false));
}

#[test]
fn environment_values_reach_the_shorthand_form_too() {
    let parsed = run(
        &["nightly"],
        &[("SNAPALL_ONLY", "home"), ("SNAPALL_NO_DBUS", "1")],
    )
    .unwrap();
    assert_eq!(parsed.request, create(Some(&["home"]), "nightly", false));
    assert!(parsed.no_dbus);
}

#[test]
fn a_scoped_environment_variable_does_not_leak_into_other_commands() {
    // SNAPALL_JSON applies to the commands that print results; the TUI and the generators
    // must ignore it.
    let parsed = run(&["tui"], &[("SNAPALL_JSON", "1")]).unwrap();
    assert!(!parsed.json);
    let parsed = run(&["man"], &[("SNAPALL_JSON", "1")]).unwrap();
    assert!(!parsed.json);
    let parsed = run(&["delete", "home", "3"], &[("SNAPALL_JSON", "1")]).unwrap();
    assert!(
        parsed.json,
        "delete prints a result too, so it takes --json"
    );
    let parsed = run(&["list"], &[("SNAPALL_JSON", "1")]).unwrap();
    assert!(parsed.json);
}

#[test]
fn an_invalid_environment_value_is_an_error_naming_where_it_came_from() {
    let message = run(&["list"], &[("SNAPALL_JSON", "maybe")])
        .unwrap_err()
        .to_string();
    assert!(message.contains("environment"), "{message}");
}

#[test]
fn a_misspelled_snapall_variable_is_an_error_with_a_suggestion() {
    // argenv's default: a near-miss of a declared variable is a typo, and a typo that is
    // ignored means the setting the user asked for is not applied.
    let message = run(&["list"], &[("SNAPALL_NO_DBU", "1")])
        .expect_err("a misspelled variable must be refused")
        .to_string();
    assert!(
        message.contains("did you mean SNAPALL_NO_DBUS"),
        "{message}"
    );
}

#[test]
fn unrelated_environment_variables_are_never_mentioned() {
    let parsed = run(
        &["list"],
        &[("PATH", "/bin"), ("HOME", "/root"), ("SNAPPER_X", "1")],
    )
    .unwrap();
    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
}

// ---- the other commands -------------------------------------------------------

#[test]
fn list_snapshots_tui_and_contract() {
    assert_eq!(ok(&["list"]).request, Request::List);
    assert_eq!(ok(&["tui"]).request, Request::Tui { file: None });
    assert_eq!(ok(&["contract"]).request, Request::Contract);
    assert_eq!(
        ok(&["snapshots"]).request,
        Request::Snapshots { config: None }
    );
    assert_eq!(
        ok(&["snapshots", "home", "--json"]),
        Parsed {
            request: Request::Snapshots {
                config: Some(names(&["home"]).remove(0))
            },
            no_dbus: false,
            json: true,
            hints: true,
            logging: Logging::default(),
            warnings: vec![],
        }
    );
}

#[test]
fn delete_needs_a_area_and_a_nonzero_id() {
    assert_eq!(
        ok(&["delete", "home", "12"]).request,
        Request::Delete {
            config: AreaName::parse("home").unwrap(),
            id: SnapshotId::new(12).unwrap(),
            destroy: false,
        }
    );
    // only --destroy answers for a deletion: --yes is for changes that can be put back
    assert_eq!(
        ok(&["delete", "home", "12", "--destroy"]).request,
        Request::Delete {
            config: AreaName::parse("home").unwrap(),
            id: SnapshotId::new(12).unwrap(),
            destroy: true,
        }
    );
    assert!(
        err(&["delete", "home", "12", "--yes"]).contains("--yes"),
        "--yes does not apply to delete"
    );
    assert!(err(&["delete"]).contains("missing required argument <AREA>"));
    assert!(err(&["delete", "home"]).contains("missing required argument <ID>"));
    assert!(err(&["delete", "home", "0"]).contains("current system"));
    assert!(err(&["delete", "home", "twelve"]).contains("snapshot_id"));
    // A negative number looks like a flag to every argv parser, argenv's included.
    assert!(err(&["delete", "home", "-3"]).contains("-3 is not a declared flag"));
}

#[test]
fn delete_arguments_are_required_by_delete_and_demanded_of_nothing_else() {
    // `required: true` on a scoped input: argenv demands it only when its own command runs.
    assert!(run(&["list"], &[]).is_ok());
    assert!(run(&["create"], &[]).is_ok());
    assert!(run(&["snapshots"], &[]).is_ok());
    assert!(run(&["delete", "home", "12"], &[]).is_ok());
}

// ---- refusing what would be silently ignored ----------------------------------

#[test]
fn a_flag_that_does_not_apply_to_the_command_is_an_error_not_ignored() {
    // The dangerous one: `--dry-run` must never be silently dropped on a delete.
    let message = err(&["delete", "home", "3", "--dry-run"]);
    assert!(
        message.contains("--dry-run does not apply to `delete`"),
        "{message}"
    );
    assert!(err(&["list", "--only", "root"]).contains("does not apply to `list`"));
    assert!(err(&["tui", "--json"]).contains("does not apply to `tui`"));
    assert!(err(&["completions", "bash", "--json"]).contains("does not apply"));
    // `--dry-run` is create's and restore's, and `restore` takes it; `delete` still does not.
    assert!(run(&["restore", "home", "3", "--dry-run"], &[]).is_ok());
    // `--yes` is doctor's and restore's, not create's.
    assert!(err(&["create", "--yes"]).contains("does not apply to `create`"));
}

#[test]
fn unknown_flags_are_errors_with_a_suggestion() {
    let message = err(&["--dry-ru"]);
    assert!(
        message.contains("--dry-ru") && message.contains("--dry-run"),
        "{message}"
    );
    assert!(err(&["list", "--evil"]).contains("--evil"));
}

#[test]
fn missing_and_surplus_values_are_errors() {
    assert!(err(&["create", "--only"]).contains("needs a value"));
    assert!(err(&["list", "--json=yes"]).contains("takes no value"));
}

#[test]
fn surplus_positionals_are_errors_derived_from_the_contract() {
    assert!(err(&["list", "extra"]).contains("unexpected argument(s): `extra`"));
    assert!(err(&["create", "a", "b"]).contains("`b`"));
    assert!(err(&["snapshots", "home", "root"]).contains("`root`"));
    assert!(err(&["delete", "home", "3", "4"]).contains("`4`"));
    assert!(err(&["tui", "x"]).contains("`x`"));
}

#[test]
fn several_problems_are_all_reported_at_once() {
    let UsageError(lines) = run(&["list", "--evil", "extra"], &[]).unwrap_err();
    assert!(lines.len() >= 2, "{lines:?}");
}

// ---- help and version ---------------------------------------------------------

#[test]
fn help_and_version_in_every_spelling() {
    assert_eq!(ok(&["--help"]).request, Request::Help);
    assert_eq!(ok(&["-h"]).request, Request::Help);
    assert_eq!(ok(&["--version"]).request, Request::Version);
    assert_eq!(ok(&["-V"]).request, Request::Version);
    assert_eq!(ok(&["list", "--help"]).request, Request::Help);
}

#[test]
fn doctor_and_its_flags() {
    assert_eq!(
        ok(&["doctor"]).request,
        Request::Doctor {
            install: false,
            yes: false
        }
    );
    assert_eq!(
        ok(&["doctor", "--install", "-y"]).request,
        Request::Doctor {
            install: true,
            yes: true
        }
    );
    assert!(ok(&["doctor", "--json"]).json);
    assert!(err(&["doctor", "--yes"]).contains("--yes only applies together with --install"));
    assert!(err(&["list", "--install"]).contains("does not apply to `list`"));
    assert!(err(&["create", "--yes"]).contains("does not apply to `create`"));
}

#[test]
fn completions_take_a_shell_and_man_takes_a_date_from_the_environment() {
    use argenv_resolve::generate::Shell;
    for (word, shell) in [
        ("bash", Shell::Bash),
        ("zsh", Shell::Zsh),
        ("fish", Shell::Fish),
        ("nushell", Shell::Nushell),
    ] {
        assert_eq!(
            ok(&["completions", word]).request,
            Request::Completions { shell }
        );
    }
    assert!(err(&["completions"]).contains("missing required argument <SHELL>"));
    assert!(err(&["completions", "powershell"]).contains("powershell"));
    assert!(err(&["completions", "bash", "zsh"]).contains("unexpected argument"));
    assert!(err(&["completions", "bash", "--json"]).contains("does not apply to `completions`"));

    assert_eq!(
        ok(&["man"]).request,
        Request::Man {
            source_date_epoch: None
        }
    );
    let reproducible = run(&["man"], &[("SOURCE_DATE_EPOCH", "1761187392")]).unwrap();
    assert_eq!(
        reproducible.request,
        Request::Man {
            source_date_epoch: Some(1_761_187_392)
        }
    );
    assert!(
        run(&["man"], &[("SOURCE_DATE_EPOCH", "yesterday")]).is_err(),
        "not a number"
    );
    // the variable belongs to `man`: it must not leak into other commands
    assert!(run(&["list"], &[("SOURCE_DATE_EPOCH", "yesterday")]).is_ok());
}

#[test]
fn changes_and_restore_take_a_area_a_snapshot_and_an_optional_path() {
    let home = AreaName::parse("home").unwrap();
    let id = SnapshotId::new(12).unwrap();
    assert_eq!(
        ok(&["changes", "home", "12"]).request,
        Request::Changes {
            config: home.clone(),
            id,
            path: None
        }
    );
    assert_eq!(
        ok(&["changes", "home", "12", "/home/me/notes.txt"]).request,
        Request::Changes {
            config: home.clone(),
            id,
            path: Some("/home/me/notes.txt".into())
        }
    );
    assert_eq!(
        ok(&["restore", "home", "12"]).request,
        Request::Restore {
            config: home.clone(),
            id,
            path: None,
            dry_run: false,
            yes: false
        }
    );
    assert_eq!(
        ok(&["restore", "home", "12", "/home/me/a", "--dry-run", "--yes"]).request,
        Request::Restore {
            config: home,
            id,
            path: Some("/home/me/a".into()),
            dry_run: true,
            yes: true,
        }
    );
}

#[test]
fn changes_and_restore_need_a_real_snapshot_and_refuse_flags_that_are_not_theirs() {
    assert!(err(&["restore", "home"]).contains("missing required argument"));
    assert!(err(&["changes"]).contains("missing required argument"));
    assert!(err(&["restore", "home", "0"]).contains("current system"));
    assert!(err(&["restore", "home", "x"]).contains("expected a non-negative integer"));
    // `--yes` belongs to doctor and restore: asking for it on a read-only command is a mistake.
    assert!(err(&["changes", "home", "3", "--yes"]).contains("does not apply to `changes`"));
    // `--dry-run` belongs to create and restore: it can never reach delete.
    assert!(err(&["delete", "home", "3", "--dry-run"]).contains("does not apply to `delete`"));
    // a fourth positional is surplus
    assert!(err(&["restore", "home", "3", "/a", "/b"]).contains("unexpected argument"));
}

#[test]
fn tips_are_on_by_default_and_can_be_silenced_by_flag_or_variable() {
    assert!(ok(&["create"]).hints);
    assert!(!ok(&["create", "--no-hints"]).hints);
    assert!(!ok(&["tui", "--no-hints"]).hints);
    assert!(!run(&["create"], &[("SNAPALL_HINTS", "off")]).unwrap().hints);
    assert!(
        run(&["create", "--hints"], &[("SNAPALL_HINTS", "off")])
            .unwrap()
            .hints,
        "the flag wins"
    );
    assert!(
        run(&["create"], &[("SNAPALL_HINTS", "maybe")]).is_err(),
        "not a boolean"
    );
    // tips only exist where a snapshot is made or browsed
    assert!(err(&["list", "--no-hints"]).contains("does not apply to `list`"));
}

#[test]
fn no_dbus_and_json_flags() {
    let parsed = ok(&["--no-dbus", "list", "--json"]);
    assert!(parsed.no_dbus && parsed.json);
}

// ---- the contract itself ------------------------------------------------------

#[test]
fn the_declaration_has_no_problems() {
    let problems = Contract::problems();
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_command_has_a_summary_and_a_unique_token() {
    let mut tokens: Vec<&str> = commands().map(|(t, _)| t).collect();
    assert!(commands().all(|(_, s)| !s.is_empty()));
    tokens.sort_unstable();
    tokens.dedup();
    assert_eq!(tokens.len(), Command::ALL.len());
    for command in Command::ALL {
        assert_eq!(Command::from_token(command.token()), Some(*command));
    }
}

#[test]
fn scoping_in_the_contract_matches_the_commands_that_exist() {
    let known: Vec<&str> = Command::ALL.iter().map(|c| c.token()).collect();
    for record in Contract::records() {
        for scope in &record.subcommands {
            assert!(
                known.contains(&scope.as_str()),
                "{}: unknown scope {scope}",
                record.key
            );
        }
    }
}

#[test]
fn edit_distance_is_levenshtein() {
    assert_eq!(edit_distance("", ""), 0);
    assert_eq!(edit_distance("lst", "list"), 1);
    assert_eq!(edit_distance("kitten", "sitting"), 3);
    assert_eq!(edit_distance("abc", "abc"), 0);
}

// ---- logging ------------------------------------------------------------------

fn logging_of(args: &[&str], env: &[(&str, &str)]) -> Logging {
    run(args, env).unwrap().logging
}

#[test]
fn logging_is_off_unless_asked_for() {
    assert_eq!(logging_of(&["list"], &[]), Logging::default());
}

#[test]
fn log_goes_to_stderr_in_the_chosen_format() {
    assert_eq!(
        logging_of(&["--log", "json", "create"], &[]),
        Logging {
            format: LogFormat::Json,
            file: None
        }
    );
    assert_eq!(
        logging_of(&["list", "--log", "text"], &[]).format,
        LogFormat::Text
    );
}

#[test]
fn a_log_file_alone_means_json_and_an_explicit_format_wins() {
    let file = Some(PathBuf::from("/var/log/snapall.jsonl"));
    assert_eq!(
        logging_of(&["--log-file", "/var/log/snapall.jsonl", "create"], &[]),
        Logging {
            format: LogFormat::Json,
            file: file.clone()
        }
    );
    assert_eq!(
        logging_of(
            &[
                "--log",
                "text",
                "--log-file",
                "/var/log/snapall.jsonl",
                "create"
            ],
            &[]
        ),
        Logging {
            format: LogFormat::Text,
            file
        }
    );
}

#[test]
fn logging_can_come_from_the_environment_and_the_flag_wins() {
    assert_eq!(
        logging_of(&["list"], &[("SNAPALL_LOG", "text")]).format,
        LogFormat::Text
    );
    assert_eq!(
        logging_of(&["list", "--log", "json"], &[("SNAPALL_LOG", "text")]).format,
        LogFormat::Json
    );
    let from_env = logging_of(&["list"], &[("SNAPALL_LOG_FILE", "/tmp/x.log")]);
    assert_eq!(from_env.file, Some(PathBuf::from("/tmp/x.log")));
    assert_eq!(from_env.format, LogFormat::Json);
}

#[test]
fn a_log_file_with_logging_switched_off_is_a_contradiction() {
    assert!(err(&["--log", "off", "--log-file", "/tmp/x", "list"])
        .contains("--log-file has no effect with --log off"));
}

#[test]
fn the_tui_can_only_log_to_a_file_because_it_owns_the_terminal() {
    let message = err(&["tui", "--log", "text"]);
    assert!(
        message.contains("cannot also log to it") && message.contains("--log-file"),
        "{message}"
    );
    assert_eq!(
        logging_of(&["tui", "--log-file", "/tmp/x.log"], &[]).file,
        Some(PathBuf::from("/tmp/x.log"))
    );
    assert!(run(&["tui"], &[]).is_ok(), "no logging at all is fine");
}

#[test]
fn an_unknown_log_format_and_logging_on_contract_are_refused() {
    assert!(!err(&["--log", "xml", "list"]).is_empty());
    assert!(err(&["contract", "--log", "json"]).contains("does not apply to `contract`"));
}

fn lint_args(argv: &[&str]) -> Vec<Finding> {
    let args: Vec<String> = argv.iter().map(|s| (*s).to_owned()).collect();
    let env: BTreeMap<String, String> = BTreeMap::new();
    let records = Contract::records();
    let resolution = Invocation {
        args: &args,
        env: &env,
    }
    .resolve(&records);
    lint(&records, &resolution)
}

// ---- characterization: the argenv behaviour snapall relies on -------------------
//
// These pin what argenv 0.2 does today. If one fails after an argenv upgrade, the
// upstream behaviour changed and the matching workaround in `check` can be simplified.

#[test]
fn argenv_demands_a_scoped_required_input_only_for_its_own_command() {
    // snapall declares `delete`'s arguments `required: true` and relies on this: before
    // argenv 0.3 a scoped required input made lint reject every other command.
    let records = Contract::records();
    assert!(
        records
            .iter()
            .any(|r| r.required && !r.subcommands.is_empty()),
        "the contract uses a scoped required input"
    );
    for command in ["list", "create", "snapshots", "tui", "doctor"] {
        let findings = lint_args(&[command]);
        assert!(
            !findings
                .iter()
                .any(|f| matches!(f, Finding::MissingRequired { .. })),
            "{command}: {findings:?}"
        );
    }
    let findings = lint_args(&["delete"]);
    assert!(
        findings
            .iter()
            .any(|f| matches!(f, Finding::MissingRequired { .. })),
        "delete without its arguments is reported: {findings:?}"
    );
}

#[test]
fn the_published_contract_says_what_each_command_requires() {
    let required: Vec<(String, Vec<String>)> = Contract::records()
        .into_iter()
        .filter(|r| r.required)
        .map(|r| (r.key, r.subcommands))
        .collect();
    assert_eq!(
        required,
        [
            (
                "config".to_owned(),
                vec![
                    "delete".to_owned(),
                    "changes".to_owned(),
                    "restore".to_owned()
                ]
            ),
            (
                "snapshot_id".to_owned(),
                vec![
                    "delete".to_owned(),
                    "changes".to_owned(),
                    "restore".to_owned()
                ]
            ),
            ("history_file".to_owned(), vec!["history".to_owned()]),
            ("shell".to_owned(), vec!["completions".to_owned()]),
        ]
    );
}

#[test]
fn history_takes_a_file_and_the_tui_can_be_opened_on_one() {
    assert_eq!(
        ok(&["history", "/home/me/notes.txt"]).request,
        Request::History {
            path: "/home/me/notes.txt".into()
        }
    );
    assert!(err(&["history"]).contains("missing required argument"));
    assert!(ok(&["history", "f", "--json"]).json);
    assert_eq!(
        ok(&["tui", "--file", "notes.txt"]).request,
        Request::Tui {
            file: Some("notes.txt".into())
        }
    );
    // --file belongs to the TUI alone, and a history is read, so --dry-run is not for it
    assert!(err(&["list", "--file", "x"]).contains("does not apply to `list`"));
    assert!(err(&["history", "f", "--dry-run"]).contains("does not apply to `history`"));
}

#[test]
fn every_command_that_reads_or_changes_snapshots_can_be_logged_restore_above_all() {
    for command in [
        "restore home 1 --yes",
        "changes home 1",
        "history /x",
        "delete home 1",
    ] {
        let mut args: Vec<&str> = command.split(' ').collect();
        args.extend(["--log", "json"]);
        assert!(
            run(&args, &[]).is_ok(),
            "`{command} --log json` must be accepted"
        );
    }
    assert!(run(
        &["restore", "home", "1", "--yes", "--log-file", "/tmp/a.log"],
        &[]
    )
    .is_ok());
}

#[test]
fn argenv_takes_unknown_flags_and_out_of_place_flags_as_errors_by_default() {
    for argv in [
        &["list", "--evil"][..],
        &["delete", "home", "3", "--dry-run"],
    ] {
        let findings = lint_args(argv);
        assert!(
            findings
                .iter()
                .any(|f| LintPolicy::default().severity(f) == Severity::Error
                    && matches!(f, Finding::UnknownArg { .. } | Finding::OutOfScope { .. })),
            "{argv:?}: {findings:?}"
        );
    }
}

#[test]
fn argenv_lists_need_a_closed_token_set_which_is_why_only_is_a_string() {
    use argenv_resolve::{Arg, Env, Input, Type};
    let dynamic_list: Input<String> = Input {
        key: "only",
        ty: Type::List,
        separators: &[','],
        env: Some(Env::new("X_ONLY")),
        arg: Some(Arg::long("only")),
        ..Input::EMPTY
    };
    assert!(
        dynamic_list
            .check()
            .iter()
            .any(|p| p.contains("`allowed` is empty")),
        "argenv now accepts open lists; `--only` could become a real list"
    );
}
