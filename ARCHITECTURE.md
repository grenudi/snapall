# Architecture

snapall follows the [software-engineering-canon](https://github.com/creation-guidelines/software-engineering-canon/blob/main/README.md). This page records which canon entries it applies and where to find them in the code. Decisions and their reasoning live in [docs/adr/](docs/adr/).

| Canon entry | How snapall applies it | Where |
|---|---|---|
| [Hexagonal architecture](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#hexagonal) | The core talks to the outside world only through one port, `SnapshotBackend`. Two **driving** adapters (the CLI and the TUI) call the use cases; one **driven** adapter (snapper) and an in-memory one implement the port. | `snapall-core/src/port.rs`, `snapall-tui`, `snapall-snapper` |
| [Functional core, imperative shell](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#functional-core-imperative-shell) | `plan` decides what to snapshot, purely. The TUI is `update(state, msg) -> effects`, a pure function; a thin session runs the effects and feeds results back as messages. `main.rs` is the shell for the CLI. | `plan.rs`, `snapall-tui/src/model.rs`, `session.rs`, `main.rs` |
| [Modular monolith](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#modular-monolith) | One deployable binary built from four crates with enforced boundaries. | workspace layout |
| Distro support as data | `doctor` decides purely: `OsRelease::parse` -> `Family` -> `InstallPlan` (steps, or a manual explanation where a command would be wrong). The shell probes the machine and runs the steps after the user agrees. Each distribution is a unit test over a real `os-release`; CI then runs the real package manager in a fresh container for six distributions. | `snapall-core/src/doctor.rs`, `snapall/src/doctor.rs`, `packaging/test/` |
| [Make illegal states unrepresentable](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#illegal-states-unrepresentable) | A `Plan` is built only by `plan()`: never empty, sorted, unique, only existing configs. A `SnapshotId` cannot be 0 (snapper's "current system", whose `delete` prints an error yet exits 0). A `Post` snapshot always carries its `Pre`. The TUI's `Load<T>` separates "not loaded", "loaded" and "failed". | `plan.rs`, `domain.rs`, `model.rs` |
| [Parse, don't validate](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#parse-dont-validate) | argv, environment and snapper output are parsed into domain types once, at the edge. | `snapall/src/cli.rs`, `snapall-snapper` |
| [Newtypes and typed primitives](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#newtypes) | `ConfigName`, `Description`, `SnapshotId`. A `ConfigName` never starts with `-`, so it is always safe as one argv element. | `domain.rs` |
| [Total functions and explicit errors](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#total-functions) | No panics on user input or snapper output; `PlanError`, `BackendError`, `ParseError`, `UsageError` are explicit. A reader that closes the pipe is not an error. | all crates, `main.rs` `emit` |
| [Immutability by default](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#immutability) | Domain values have private fields and no mutators. | `domain.rs` |
| [Test at the seams](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#test-at-the-seams) | Core: in-memory adapter. Snapper adapter: scripted commands, parsing tested on output captured from a real snapper. TUI: key sequences against the in-memory backend, and rendering into ratatui's test terminal. CLI: pure `parse(args, env)` with the environment injected. Contract: one `backend_contract` run against the in-memory backend and real snapper. Few end-to-end checks: the real binary, and the real TUI in a pseudo-terminal. | `testing.rs`, `tests/`, `dev/tui-e2e.py`, CI `integration` |
| [Enforce boundaries with tooling](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#enforce-boundaries) | Crate boundaries, plus a test over `cargo metadata`: the core depends on `thiserror` only; the adapters and the TUI never know each other; only the binary knows argenv; nobody uses a CLI parser other than argenv. Clippy denies `unwrap`, `expect` and `panic` in production code of every crate (test code may use them), so "total functions" is enforced, not remembered. | `snapall/tests/architecture.rs` |
| [Evolutionary architecture with fitness functions](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#evolutionary-architecture) | The same test, run by CI on every change, plus characterization tests that pin the third-party behaviour snapall works around, so an upgrade that changes it fails loudly. | CI `test` job, `cli.rs` tests |
| [CI as the gatekeeper](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#ci-gatekeeper) | Format, clippy with warnings denied, tests, docs with warnings denied, MSRV, advisories, real snapper, the real TUI, `argenv lint` on the published contract, licenses, and the real packages on fresh containers of each distribution on x86_64 and aarch64. After every release, `release-verify.yml` repeats the install checks on the *published* assets. | `.github/workflows/ci.yml`, `release-verify.yml` |
| [Architecture decision records](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#adr) | One ADR per significant decision. | `docs/adr/` |
| [Observability as a port](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#observability) | The core defines the port: `Event` (pure data: what was done, how it went, how long it took) and `Observer`. The `snapall-observe` adapter is a decorator, `Observed`, that wraps any `SnapshotBackend`, times each call and reports it, so no use case knows about logging and the wrapped backend still passes the same contract. `Lines` writes text or JSON Lines, to stderr or an append-only 0600 file; a sink that fails never fails the operation. The clock lives in the adapter, not the core. | `snapall-core/src/observe.rs`, `snapall-observe` |
| [Twelve-factor discipline](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#twelve-factor) | Configuration: every setting has a flag *and* a `SNAPALL_*` variable, from one declaration (argenv). Logs: an event stream on stderr, not a file the program manages. Disposability: SIGTERM, SIGHUP and SIGINT stop the TUI cleanly, restore the terminal and exit `128 + signal`, proved in a pseudo-terminal. | `contract.rs`, `snapall-tui/src/term.rs`, `dev/tui-e2e.py` |
| [Typestate](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#typestate) | `doctor --install` can only run steps that are `Approved`, and the only way to get an `Approved` is to show them and be told yes (or `--yes`). Running unapproved package-manager commands as root is not a state the program can be in. | `snapall/src/doctor.rs` |
| [Domain-driven design](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#domain-driven-design) | One vocabulary, written down: [docs/glossary.md](docs/glossary.md). Four contexts with their own models: snapshots (`snapall-core`), environment diagnosis (`doctor`), invocation (the argenv contract) and presentation (the TUI). | `docs/glossary.md` |

## The invocation contract

The CLI surface (flags, positionals, environment variables, which command each applies to) is declared once with [argenv](https://github.com/argenv-opencommons/argenv) in `crates/snapall/src/contract.rs`. From that single declaration come: argument and environment resolution with provenance, `--help`, the validation of invocations, and the published `snapall contract` document (committed as `snapall.argenv.json`, checked by a test and by `argenv lint` in CI). The canon's DRY applied to an interface: one authoritative home for each piece of knowledge.

## The dependency rule

```
snapall (binary: argenv CLI, wiring, rendering)
   ├── snapall-tui ───────▶ snapall-core     (+ ratatui, signal-hook)
   ├── snapall-observe ───▶ snapall-core     (+ serde_json)
   ├── snapall-snapper ───▶ snapall-core     (+ serde_json)
   ├── snapall-system ────▶ snapall-core     (+ serde_json: df, systemctl, unit files, quotas, the lock, the journal, the runner)
   └────────────────────────▶ snapall-core     (no I/O, no parser, no terminal, no clock)
```

Dependencies point inward. `snapall-core` knows nothing about snapper, JSON, processes, terminals, clocks or the command line, and the outer crates never see each other; they meet only in the binary.

## The window, module by module

`snapall-tui` is the functional core and imperative shell in miniature, and each file says one thing:

```
model/            what the window does (pure: no terminal, no clock, no I/O)
   input.rs         a key, a click, a button
   state.rs         what it knows
   messages.rs      what it asks the shell to do, and what the shell reports back
   keys.rs          what each key and click means
   lists.rs file.rs the arrival of the lists; the file-history screen
   prompts/         one module per prompt: describe, delete, restore, points, protect, help, limits, groups,
                    unprotect, and confirm: the one question every change goes through
tabs/             the Health and Activity tabs: each owns its state, keys and drawing
status_bar.rs     the line under every view
view/             drawing only: the frame, the tab bar, the lists, the footer, where a click lands,
                  and prompts/ (one module per prompt's drawing)
session.rs        the shell: performs effects, stamps changes for the Activity tab
term.rs           the terminal: keys in, frames out, a worker thread for slow calls
```

A test fails if any file of the window grows past 450 lines of code: a file that long is doing more than one thing.

