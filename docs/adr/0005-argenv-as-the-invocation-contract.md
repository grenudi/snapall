# 5. argenv as the invocation contract

Status: accepted. Supersedes [ADR 3](0003-clap-now-argenv-later.md).

## Context
`argenv` (same author) declares a program's argv and environment as typed, machine-readable data, and resolves invocations against it with provenance. ADR 3 deferred adopting it because its API had not been read. It has now been read, from source, and used.

## Decision
The whole invocation surface lives in `crates/snapall/src/contract.rs`. `clap` is removed. Parsing, `--help`, validation and the published `snapall contract` document all derive from that file.

Since argenv 0.3 almost all of the policy a tool that changes the system needs is argenv's own default, so snapall's layer (`cli.rs`) is thin:

- **Normalise first.** `snapall "text"` is rewritten to `snapall create "text"` before resolving, so subcommand-scoped rules (environment variables, positionals, applicable flags) behave identically for both spellings, and a one-word typo of a command is refused instead of becoming a description.
- **argenv's default policy.** A misspelled flag, a misspelled variable, and a flag given to a subcommand it does not belong to (`delete --dry-run`) are errors. snapall used to implement those itself because argenv 0.2 treated them as warnings or accepted them silently; the decisions and the code moved upstream (argenv#30, #31) and snapall's copies were deleted.
- **`required` works per command.** `delete`'s arguments are declared `required: true`, and argenv demands them only when `delete` runs (argenv#22), so the published contract says what `delete` needs and `argenv lint` accepts every other command.
- **Own the exit.** `lint` is called directly instead of `handle_problems`, which calls `process::exit`, so the exit status is snapall's (0, 1, 2).
- **`--only` is a string.** argenv lists need a non-empty closed `allowed` set; config names are only known at runtime (argenv#25 is open). snapall splits and validates it itself.
- **Extra positionals** are counted from the contract (argenv does not report them).

A project that wanted to be more forgiving would pass `LintPolicy::lenient()`; snapall deliberately does not, because a destructive tool should not carry on past a typo.

The argenv behaviour snapall *relies on* is pinned by tests that fail if it changes (`cli.rs`): scoped `required`, unknown and out-of-place flags as errors, and the closed-list rule behind `--only`.

## Consequences
The contract is checked in CI by argenv's own CLI, pinned to a release tag: `argenv check` (the document is valid), `argenv lint` accepting valid invocations and rejecting invalid ones, and `--lenient` staying lenient only about typos. The one open upstream item is argenv#25.
