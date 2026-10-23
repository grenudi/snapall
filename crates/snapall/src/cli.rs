// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Driving adapter: turn an argv and an environment into a validated [`Parsed`] request.
//!
//! Everything here is a pure function of its inputs (no process state), so every rule is a
//! plain test. argenv resolves and lints; this module decides policy and parses the
//! survivors into domain types, once, at the boundary.

use std::fmt;

use argenv_resolve::{
    lint, EnvSource, Finding, Input, InputResolveDefaultExt, InputResolveExt, Invocation, Record,
    Resolution, Source,
};
use snapall_core::{ConfigName, Description, SnapshotId};

use crate::contract::{self, Command, Contract};

/// What the user asked for, fully parsed.
#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Help,
    Version,
    Create {
        only: Option<Vec<ConfigName>>,
        description: Description,
        dry_run: bool,
    },
    List,
    Snapshots {
        config: Option<ConfigName>,
    },
    Delete {
        config: ConfigName,
        id: SnapshotId,
    },
    Doctor {
        install: bool,
        yes: bool,
    },
    Tui,
    Contract,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Parsed {
    pub request: Request,
    pub no_dbus: bool,
    pub json: bool,
    /// Problems worth showing that do not stop the run (for example a misspelled
    /// `SNAPALL_*` variable).
    pub warnings: Vec<String>,
}

/// The invocation is wrong. Reported with exit status 2.
#[derive(Debug, PartialEq, Eq)]
pub struct UsageError(pub Vec<String>);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("\n"))
    }
}

impl std::error::Error for UsageError {}

fn usage_error(message: impl Into<String>) -> UsageError {
    UsageError(vec![message.into()])
}

/// Parse an argument vector (without the program name) and an environment.
pub fn parse(args: &[String], env: &dyn EnvSource) -> Result<Parsed, UsageError> {
    let records = Contract::records();

    // `snapall "before theming"` is shorthand for `snapall create "before theming"`. Make it
    // explicit *before* resolving, so every subcommand-scoped rule (environment variables,
    // positionals, applicable flags) works the same for both spellings.
    let first_pass = Invocation { args, env }.resolve(&records);
    let args = with_explicit_command(args, first_pass.positionals())?;
    let resolution = Invocation { args: &args, env }.resolve(&records);

    let command = resolution
        .positionals()
        .first()
        .and_then(|token| Command::from_token(token))
        .expect("with_explicit_command guarantees a command token");

    let (errors, warnings) = check(&records, &resolution, command);
    if !errors.is_empty() {
        return Err(UsageError(errors));
    }

    let request = if flag(&Contract::HELP, &resolution) {
        Request::Help
    } else if flag(&Contract::VERSION, &resolution) {
        Request::Version
    } else {
        build_request(command, &resolution, flag(&Contract::DRY_RUN, &resolution))?
    };
    Ok(Parsed {
        request,
        no_dbus: flag(&Contract::NO_DBUS, &resolution),
        json: flag(&Contract::JSON, &resolution),
        warnings,
    })
}

/// A boolean input: what was given, else its declared default, else off.
fn flag<C: 'static>(input: &Input<bool, C>, resolution: &Resolution) -> bool {
    input.get_from_or_default(resolution).unwrap_or(false)
}

/// Ensure the first positional is a command token, inserting `create` when it is not.
fn with_explicit_command(
    args: &[String],
    positionals: &[String],
) -> Result<Vec<String>, UsageError> {
    let mut explicit = args.to_vec();
    match positionals.first() {
        Some(token) if Command::from_token(token).is_some() => {}
        Some(token) => {
            // A one-word typo of a command must not silently become a snapshot description.
            if let Some(near) = nearest_command(token) {
                return Err(usage_error(format!(
                    "`{token}` is not a command; did you mean `{near}`? \
                     (to use it as a description, write `snapall create {token}`)"
                )));
            }
            explicit.insert(0, Command::Create.token().to_owned());
        }
        None => explicit.insert(0, Command::Create.token().to_owned()),
    }
    Ok(explicit)
}

fn nearest_command(word: &str) -> Option<&'static str> {
    let budget = if word.chars().count() <= 4 { 1 } else { 2 };
    Command::ALL
        .iter()
        .map(|c| (edit_distance(word, c.token()), c.token()))
        .filter(|(distance, _)| *distance <= budget)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, token)| token)
}

/// Levenshtein distance, two-row variant.
fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

fn applies_to(record: &Record, command: Command) -> bool {
    record.subcommands.is_empty() || record.subcommands.iter().any(|s| s == command.token())
}

/// Apply snapall's policy to argenv's findings. Returns (errors, warnings).
///
/// argenv treats an unknown flag as a warning and has no notion of a flag that is valid for
/// one subcommand only. A tool that deletes things cannot be that forgiving: a misspelled
/// flag, or `--dry-run` on a command that does not honour it, is an error.
fn check(
    records: &[Record],
    resolution: &Resolution,
    command: Command,
) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for finding in lint(records, resolution) {
        match &finding {
            Finding::UnknownEnv { .. } | Finding::Deprecated { .. } => {
                warnings.push(finding.to_string());
            }
            _ => errors.push(finding.to_string()),
        }
    }

    // A flag given for a command it does not belong to.
    for record in records {
        let is_flag = record.arg.as_ref().is_some_and(|a| a.position.is_none());
        if is_flag
            && !applies_to(record, command)
            && resolution.source(&record.key) == Some(Source::Arg)
        {
            errors.push(format!(
                "{} does not apply to `{}` (it applies to: {})",
                record.usage(),
                command.token(),
                record.subcommands.join(", ")
            ));
        }
    }

    // More positionals than this command declares: derived from the contract, not hard-coded.
    let declared = records
        .iter()
        .filter(|r| applies_to(r, command) && !r.subcommands.is_empty())
        .filter(|r| r.arg.as_ref().is_some_and(|a| a.position.is_some()))
        .count();
    let extras: Vec<&String> = resolution.positionals().iter().skip(1 + declared).collect();
    if !extras.is_empty() {
        let list: Vec<String> = extras.iter().map(|e| format!("`{e}`")).collect();
        errors.push(format!("unexpected argument(s): {}", list.join(", ")));
    }

    (errors, warnings)
}

fn build_request(
    command: Command,
    resolution: &Resolution,
    dry_run: bool,
) -> Result<Request, UsageError> {
    Ok(match command {
        Command::Create => Request::Create {
            only: Contract::ONLY
                .get_from(resolution)
                .map(|raw| parse_only(&raw))
                .transpose()?,
            description: match Contract::DESCRIPTION.get_from(resolution) {
                Some(text) => Description::parse(&text)
                    .map_err(|e| usage_error(format!("description: {e}")))?,
                None => Description::default(),
            },
            dry_run,
        },
        Command::List => Request::List,
        Command::Snapshots => Request::Snapshots {
            config: Contract::SNAPSHOTS_CONFIG
                .get_from(resolution)
                .map(|name| parse_config("CONFIG", &name))
                .transpose()?,
        },
        Command::Delete => {
            let missing = |record: argenv_resolve::Record| {
                usage_error(format!("missing required argument {}", record.usage()))
            };
            let config = Contract::DELETE_CONFIG
                .get_from(resolution)
                .ok_or_else(|| missing(Contract::DELETE_CONFIG.to_record()))?;
            let id = Contract::SNAPSHOT_ID
                .get_from(resolution)
                .ok_or_else(|| missing(Contract::SNAPSHOT_ID.to_record()))?;
            Request::Delete {
                config: parse_config("CONFIG", &config)?,
                id: SnapshotId::new(id).ok_or_else(|| {
                    usage_error("ID 0 is the current system, not a snapshot; it cannot be deleted")
                })?,
            }
        }
        Command::Doctor => {
            let install = flag(&Contract::INSTALL, resolution);
            let yes = flag(&Contract::YES, resolution);
            if yes && !install {
                return Err(usage_error("--yes only applies together with --install"));
            }
            Request::Doctor { install, yes }
        }
        Command::Tui => Request::Tui,
        Command::Contract => Request::Contract,
    })
}

fn parse_config(what: &str, name: &str) -> Result<ConfigName, UsageError> {
    ConfigName::parse(name).map_err(|e| usage_error(format!("{what}: {e}")))
}

/// `--only a,b`: a comma-separated list of config names, each validated.
fn parse_only(raw: &str) -> Result<Vec<ConfigName>, UsageError> {
    raw.split(',')
        .map(|name| parse_config("--only", name.trim()))
        .collect()
}

/// Every command's one-line summary must exist; used by `--help` and checked by a test.
pub fn commands() -> impl Iterator<Item = (&'static str, &'static str)> {
    contract::Command::ALL
        .iter()
        .map(|c| (c.token(), c.summary()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

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

    fn names(list: &[&str]) -> Vec<ConfigName> {
        list.iter().map(|n| ConfigName::parse(n).unwrap()).collect()
    }

    fn create(only: Option<&[&str]>, description: &str, dry_run: bool) -> Request {
        Request::Create {
            only: only.map(names),
            description: Description::parse(description).unwrap(),
            dry_run,
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
    fn bad_descriptions_and_config_names_are_rejected_at_the_boundary() {
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
        // SNAPALL_JSON applies to create/list/snapshots only; deleting must ignore it.
        let parsed = run(&["delete", "home", "3"], &[("SNAPALL_JSON", "1")]).unwrap();
        assert!(!parsed.json);
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
    fn a_misspelled_snapall_variable_is_a_warning_with_a_suggestion() {
        let parsed = run(&["list"], &[("SNAPALL_NO_DBU", "1")]).unwrap();
        assert_eq!(parsed.request, Request::List);
        assert!(
            !parsed.no_dbus,
            "the misspelled variable must not take effect"
        );
        assert!(
            parsed
                .warnings
                .iter()
                .any(|w| w.contains("did you mean SNAPALL_NO_DBUS")),
            "{:?}",
            parsed.warnings
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
        assert_eq!(ok(&["tui"]).request, Request::Tui);
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
                warnings: vec![],
            }
        );
    }

    #[test]
    fn delete_needs_a_config_and_a_nonzero_id() {
        assert_eq!(
            ok(&["delete", "home", "12"]).request,
            Request::Delete {
                config: ConfigName::parse("home").unwrap(),
                id: SnapshotId::new(12).unwrap(),
            }
        );
        assert!(err(&["delete"]).contains("missing required argument <CONFIG>"));
        assert!(err(&["delete", "home"]).contains("missing required argument <ID>"));
        assert!(err(&["delete", "home", "0"]).contains("current system"));
        assert!(err(&["delete", "home", "twelve"]).contains("snapshot_id"));
        // A negative number looks like a flag to every argv parser, argenv's included.
        assert!(err(&["delete", "home", "-3"]).contains("-3 is not a declared flag"));
    }

    #[test]
    fn delete_arguments_are_not_demanded_of_other_commands() {
        assert!(run(&["list"], &[]).is_ok());
        assert!(run(&["create"], &[]).is_ok());
        assert!(run(&["snapshots"], &[]).is_ok());
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
        assert!(err(&["delete", "home", "3", "--json"]).contains("does not apply"));
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

    // ---- characterization: argenv behaviours snapall works around -------------------
    //
    // These pin what argenv 0.2 does today. If one fails after an argenv upgrade, the
    // upstream behaviour changed and the matching workaround in `check` can be simplified.

    #[test]
    fn argenv_lint_ignores_subcommand_scope_for_required_inputs() {
        // This is why the contract declares no `required` input: one scoped to `delete` would
        // make `argenv lint` reject every other command.
        use argenv_resolve::{Arg, Input, Type};
        let scoped: Input<String, Command> = Input {
            key: "needed",
            ty: Type::String,
            required: true,
            subcommands: &[Command::Delete],
            arg: Some(Arg::positional(0)),
            ..Input::EMPTY
        };
        let records = vec![scoped.to_record()];
        let args = vec!["list".to_owned()];
        let env: BTreeMap<String, String> = BTreeMap::new();
        let resolution = Invocation {
            args: &args,
            env: &env,
        }
        .resolve(&records);
        assert!(
            lint(&records, &resolution)
                .iter()
                .any(|f| matches!(f, Finding::MissingRequired { .. })),
            "argenv now scopes `required`; the contract may declare `required` on delete's inputs"
        );
    }

    #[test]
    fn the_contract_declares_no_required_input_so_argenv_lint_works_for_everyone() {
        assert!(Contract::records().iter().all(|r| !r.required));
    }

    #[test]
    fn argenv_reports_an_unknown_flag_only_as_a_warning() {
        let records = Contract::records();
        let args = vec!["list".to_owned(), "--evil".to_owned()];
        let env: BTreeMap<String, String> = BTreeMap::new();
        let resolution = Invocation {
            args: &args,
            env: &env,
        }
        .resolve(&records);
        let finding = lint(&records, &resolution)
            .into_iter()
            .find(|f| matches!(f, Finding::UnknownArg { .. }))
            .expect("unknown flag is reported");
        assert_eq!(finding.severity(), argenv_resolve::Severity::Warning);
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
}
