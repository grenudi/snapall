# Architecture

snapall follows the [software-engineering-canon](https://github.com/creation-guidelines/software-engineering-canon/blob/main/README.md). This page records which canon entries it applies and where to find them in the code. Decisions and their reasoning live in [docs/adr/](docs/adr/).

| Canon entry | How snapall applies it | Where |
|---|---|---|
| [Hexagonal architecture](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#hexagonal) | The core talks to the outside world only through one port, `SnapshotBackend`. Adapters plug in: the snapper CLI, and an in-memory backend for tests. | `snapall-core/src/port.rs`, `snapall-snapper` |
| [Functional core, imperative shell](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#functional-core-imperative-shell) | `plan` is a pure, total function that decides what to snapshot. `main.rs` is the thin shell that performs I/O and maps outcomes to exit codes. | `snapall-core/src/plan.rs`, `snapall/src/main.rs` |
| [Modular monolith](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#modular-monolith) | One deployable binary built from crates with enforced boundaries. | workspace layout |
| [Make illegal states unrepresentable](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#illegal-states-unrepresentable) | A `Plan` can only be built by `plan()`, so it is never empty, unsorted or duplicated, and only names configs that exist. | `plan.rs` |
| [Parse, don't validate](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#parse-dont-validate) | CLI text and snapper output are parsed into domain types once, at the edge. Downstream code never re-checks. | `domain.rs`, `cli.rs`, `snapall-snapper` |
| [Newtypes and typed primitives](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#newtypes) | `ConfigName`, `Description` and `SnapshotId` replace raw strings and integers. A `ConfigName` can never start with `-`, so it is always safe as one argv element. | `domain.rs` |
| [Total functions and explicit errors](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#total-functions) | No panics on user input. `PlanError`, `BackendError` and `ParseError` are explicit; garbage from snapper is `UnexpectedOutput`, not a crash. | all crates |
| [Immutability by default](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/principles.md#immutability) | Domain values have private fields and no mutators. | `domain.rs` |
| [Test at the seams](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#test-at-the-seams) | Core: in-memory adapter. Adapter: scripted command runner, exact argv and parsing. Contract: one `backend_contract` run against the in-memory backend and against real snapper. Very few end-to-end checks. | `testing.rs`, `real_snapper.rs`, CI `integration` job |
| [Enforce boundaries with tooling](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#enforce-boundaries) | Crate boundaries, plus a test that reads `cargo metadata` and fails if the core depends on anything but `thiserror`, or a library depends on `clap`. | `snapall/tests/architecture.rs` |
| [Evolutionary architecture with fitness functions](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#evolutionary-architecture) | The same architecture test is the fitness function; CI runs it on every change. | CI `test` job |
| [CI as the gatekeeper](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#ci-gatekeeper) | Format, clippy with warnings denied, tests, docs with warnings denied, MSRV, advisories, and the real-snapper job. | `.github/workflows/ci.yml` |
| [Architecture decision records](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#adr) | One ADR per significant decision. | `docs/adr/` |
| [Observability as a port](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/practices.md#observability) | Deliberately deferred (YAGNI). Output is a pure render function today; a logging port is backlog. | `docs/backlog.md` |

## The dependency rule

```
snapall (binary: CLI + wiring)
   ├── snapall-snapper ──▶ snapall-core
   └──────────────────────▶ snapall-core        (no I/O, no clap, no serde)
```

Dependencies point inward. `snapall-core` knows nothing about snapper, JSON, processes or the command line.
