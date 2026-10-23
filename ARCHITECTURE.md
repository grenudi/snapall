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
| [Enforce boundaries with tooling](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#enforce-boundaries) | Crate boundaries, plus a test over `cargo metadata`: the core depends on `thiserror` only; the adapters and the TUI never know each other; only the binary knows argenv; nobody uses a CLI parser other than argenv. | `snapall/tests/architecture.rs` |
| [Evolutionary architecture with fitness functions](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#evolutionary-architecture) | The same test, run by CI on every change, plus characterization tests that pin the third-party behaviour snapall works around, so an upgrade that changes it fails loudly. | CI `test` job, `cli.rs` tests |
| [CI as the gatekeeper](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#ci-gatekeeper) | Format, clippy with warnings denied, tests, docs with warnings denied, MSRV, advisories, real snapper, the real TUI, and `argenv lint` on the published contract. | `.github/workflows/ci.yml` |
| [Architecture decision records](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#adr) | One ADR per significant decision. | `docs/adr/` |
| [Observability as a port](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#observability) | Deliberately deferred (YAGNI). Output is pure render functions today. | `docs/backlog.md` |

## The invocation contract

The CLI surface (flags, positionals, environment variables, which command each applies to) is declared once with [argenv](https://github.com/argenv-opencommons/argenv) in `crates/snapall/src/contract.rs`. From that single declaration come: argument and environment resolution with provenance, `--help`, the validation of invocations, and the published `snapall contract` document (committed as `snapall.argenv.json`, checked by a test and by `argenv lint` in CI). The canon's DRY applied to an interface: one authoritative home for each piece of knowledge.

## The dependency rule

```
snapall (binary: argenv CLI, wiring, rendering)
   ├── snapall-tui ───────▶ snapall-core     (+ ratatui)
   ├── snapall-snapper ───▶ snapall-core     (+ serde_json)
   └────────────────────────▶ snapall-core     (no I/O, no parser, no terminal)
```

Dependencies point inward. `snapall-core` knows nothing about snapper, JSON, processes, terminals or the command line, and the three outer crates never see each other.
