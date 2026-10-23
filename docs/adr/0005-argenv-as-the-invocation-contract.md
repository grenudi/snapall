# 5. argenv as the invocation contract

Status: accepted. Supersedes [ADR 3](0003-clap-now-argenv-later.md).

## Context
`argenv` (same author) declares a program's argv and environment as typed, machine-readable data, and resolves invocations against it with provenance. ADR 3 deferred adopting it because its API had not been read. It has now been read, from source, and used.

## Decision
The whole invocation surface lives in `crates/snapall/src/contract.rs`. `clap` is removed. Parsing, `--help`, validation and the published `snapall contract` document all derive from that file.

snapall adds a thin policy layer (`cli.rs`) on top of argenv, because a tool that deletes things needs stricter rules than argenv's defaults:

- **Normalise first.** `snapall "text"` is rewritten to `snapall create "text"` before resolving, so subcommand-scoped rules (environment variables, positionals) behave identically for both spellings.
- **Reject out-of-scope flags.** argenv parses flags globally, whatever subcommand they are scoped to. snapall rejects a flag whose contract record does not list the active command, so `delete ... --dry-run` is an error, not a silent real delete.
- **Unknown flags are errors.** argenv reports them as warnings.
- **Own the exit.** `lint` is called directly instead of `handle_problems`, which calls `process::exit`.
- **No `required` inputs in the contract.** argenv's lint ignores subcommand scope, so a `required` input of `delete` would make `argenv lint` reject every other command. `delete`'s arguments are enforced by snapall instead.
- **`--only` is a string.** argenv lists need a non-empty closed `allowed` set; config names are only known at runtime.

Each of these workarounds is pinned by a characterization test that fails if argenv changes, so the workaround can then be removed.

## Consequences
The contract is checked in CI by argenv's own CLI (`argenv lint`). `argenv check` is not run yet: at the pinned revision it rejects the `subcommands` field that argenv itself emits (`KNOWN_FIELDS` in `argenv-cli`). The upstream items are in `docs/backlog.md`.
