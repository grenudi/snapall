// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parsing what was typed: the command, its words, and what to suggest when it is mistyped.

use super::*;
use crate::contract::{self, Command, Contract, LogFormat};
use argenv_resolve::{
    lint, EnvSource, Finding, Input, InputResolveDefaultExt, InputResolveExt, Invocation,
    LintPolicy, Record, Resolution, Severity, Source,
};

/// Parse an argument vector (without the program name) and an environment.
pub fn parse(args: &[String], env: &dyn EnvSource) -> Result<Parsed, UsageError> {
    let records = Contract::records();

    // `snapall "before theming"` is shorthand for `snapall create "before theming"`. Make it
    // explicit *before* resolving, so every subcommand-scoped rule (environment variables,
    // positionals, applicable flags) works the same for both spellings.
    let first_pass = Invocation { args, env }.resolve(&records);
    let (args, command) = with_explicit_command(args, first_pass.positionals())?;
    let resolution = Invocation { args: &args, env }.resolve(&records);

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
    let logging = logging(&resolution, command)?;
    Ok(Parsed {
        request,
        no_dbus: flag(&Contract::NO_DBUS, &resolution),
        json: flag(&Contract::JSON, &resolution),
        hints: flag(&Contract::HINTS, &resolution),
        logging,
        warnings,
    })
}

/// Combine `--log` and `--log-file`. A log file on its own means JSON (an audit trail is for
/// machines); a log on its own goes to stderr; and the TUI, which owns the terminal, can only
/// log to a file.
pub(super) fn logging(resolution: &Resolution, command: Command) -> Result<Logging, UsageError> {
    // `get_from` also returns the declared default, so ask where the value came from: only an
    // argument or a variable counts as the user having chosen a format.
    let chosen = matches!(
        resolution.source(Contract::LOG.key),
        Some(Source::Arg | Source::Env)
    );
    let explicit = if chosen {
        Contract::LOG.get_from(resolution)
    } else {
        None
    };
    let file = Contract::LOG_FILE.get_from(resolution);
    let format = match (explicit, &file) {
        (Some(format), _) => format,
        (None, Some(_)) => LogFormat::Json,
        (None, None) => LogFormat::Off,
    };
    if format == LogFormat::Off && file.is_some() {
        return Err(usage_error("--log-file has no effect with --log off"));
    }
    if command == Command::Tui && format != LogFormat::Off && file.is_none() {
        return Err(usage_error(
            "`tui` draws on the terminal, so it cannot also log to it; add --log-file PATH",
        ));
    }
    Ok(Logging { format, file })
}

/// A boolean input: what was given, else its declared default, else off.
pub(super) fn flag<C: 'static>(input: &Input<bool, C>, resolution: &Resolution) -> bool {
    input.get_from_or_default(resolution).unwrap_or(false)
}

/// The arguments with the command made explicit, and that command: `create` is inserted when the
/// first positional is not a command token. Returning the command spares the caller from
/// re-deriving (and unwrapping) it.
pub(super) fn with_explicit_command(
    args: &[String],
    positionals: &[String],
) -> Result<(Vec<String>, Command), UsageError> {
    if let Some(command) = positionals.first().and_then(|t| Command::from_token(t)) {
        return Ok((args.to_vec(), command));
    }
    // Not a command (or nothing at all): the invocation is `create`, with this as its
    // description. A one-word typo of a command must not silently become a description.
    if let Some(token) = positionals.first() {
        if let Some(near) = nearest_command(token) {
            return Err(usage_error(format!(
                "`{token}` is not a command; did you mean `{near}`? \
                 (to use it as a description, write `snapall create {token}`)"
            )));
        }
    }
    let mut explicit = args.to_vec();
    explicit.insert(0, Command::Create.token().to_owned());
    Ok((explicit, Command::Create))
}

pub(super) fn nearest_command(word: &str) -> Option<&'static str> {
    let budget = if word.chars().count() <= 4 { 1 } else { 2 };
    Command::ALL
        .iter()
        .map(|c| (edit_distance(word, c.token()), c.token()))
        .filter(|(distance, _)| *distance <= budget)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, token)| token)
}

/// Levenshtein distance, two-row variant.
pub(super) fn edit_distance(a: &str, b: &str) -> usize {
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

pub(super) fn applies_to(record: &Record, command: Command) -> bool {
    record.subcommands.is_empty() || record.subcommands.iter().any(|s| s == command.token())
}

/// Apply the policy to argenv's findings. Returns (errors, warnings).
///
/// argenv's default policy is the right one for a tool that changes the system: a misspelled
/// flag or variable, and a flag given to a subcommand it does not belong to (`delete
/// --dry-run`), are errors, because carrying on would do something other than what was asked.
/// snapall used to enforce that itself; since argenv 0.3 it is the library's default, so this
/// only chooses the wording and sorts by severity.
pub(super) fn check(
    records: &[Record],
    resolution: &Resolution,
    command: Command,
) -> (Vec<String>, Vec<String>) {
    let policy = LintPolicy::default();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    for finding in lint(records, resolution) {
        let message = match &finding {
            // A missing positional reads better as the argument the user should add.
            Finding::MissingRequired { key, .. } => records
                .iter()
                .find(|r| &r.key == key && r.arg.as_ref().is_some_and(|a| a.position.is_some()))
                .map_or_else(
                    || finding.to_string(),
                    |r| format!("missing required argument {}", r.usage()),
                ),
            _ => finding.to_string(),
        };
        match policy.severity(&finding) {
            Severity::Error => errors.push(message),
            Severity::Warning => warnings.push(message),
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

/// Every command's one-line summary must exist; used by `--help` and checked by a test.
pub fn commands() -> impl Iterator<Item = (&'static str, &'static str)> {
    contract::Command::ALL
        .iter()
        .map(|c| (c.token(), c.summary()))
}
