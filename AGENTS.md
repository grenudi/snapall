# AGENTS.md - how to work in this repository

snapall snapshots every snapper config, from a CLI and a terminal UI. The architecture is settled and written down; **work on the behaviour, do not re-derive the structure.** Read [ARCHITECTURE.md](ARCHITECTURE.md) first.

## Commands
```
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features        # includes the architecture fitness test
```
Real snapper and the real TUI (root, loop devices, `snapper`, `btrfs-progs`, `pip install pyte`): see the README's Development section.

## Rules
1. **The dependency rule is a test, not a convention.** `snapall-core` has no I/O and depends only on `thiserror`. The TUI, the snapper adapter and the binary never depend on each other except through the binary's wiring. Only the binary knows argenv. `snapall-core` also reads no clock: time belongs to adapters. If `crates/snapall/tests/architecture.rs` fails, fix the design, not the test.
2. **The CLI surface has one home: `crates/snapall/src/contract/`.** Flags, positionals, environment variables and which command each applies to are declared there with argenv; `--help`, parsing and `snapall contract` all derive from it. After changing it, regenerate `snapall.argenv.json` (`cargo run -q -p snapall -- contract > snapall.argenv.json`); a test fails if it is stale.
3. **Parse at the boundary.** argv, environment and snapper output become `AreaName`, `Description` or `SnapshotId` once, in `cli/` or an adapter. Core code never takes a raw `&str` for these.
4. **Operations are observed, not logged by hand.** Everything that touches snapper goes through the `Observed` backend; privileged side effects outside it (the package-manager steps) report an `Event` themselves. A new `Event` needs a log-format test and an entry in `docs/glossary.md`. A log sink must never fail the operation it observes.
5. **No shell, no unvalidated argv.** Commands are argument vectors run without a shell. A value that looks like an option stays one argument.
6. **Total functions.** No `unwrap`/`expect`/panics on anything that comes from the user or from snapper. Add an explicit error variant instead. Clippy enforces it: `unwrap_used`, `expect_used` and `panic` are denied in production code (tests are exempt via `cfg_attr(test, ...)`). Write to stdout through `emit` in `main.rs`, never `println!`: a closed pipe must not panic.
7. **The TUI's behaviour lives in `snapall-tui/src/model/`** as the pure `update`, including the count of work in flight (`pending`). Drawing stays in `view/`; effects are performed only by the `Executor` in `session/` (on a worker thread in the real terminal, inline in the tests), so the screen never waits for snapper. A new key or flow is a test against `Session` first.
8. **Test at the seam where the behaviour lives.** Planning and use cases: in-memory backend. Command lines and output parsing: scripted runner. Invocation rules: `cli::parse(args, env)` with an injected environment. Whether snapper accepts a command: the contract test against real snapper. Do not guess snapper's output format; capture it from the real tool.
9. **A new backend must pass `snapall_core::ports::testing::backend_contract`.**
10. **Third-party behaviour we rely on is pinned by a characterization test** (see `cli/`). If one fails after an upgrade, the upstream behaviour changed: update the code or the test on purpose, never just the assertion.
11. **MSRV is 1.85.** `Cargo.lock` is committed and pins `instability` to 0.3.10 (see ADR 6). Do not `cargo update` past it, and do not bump ratatui without moving the MSRV deliberately.
12. **`doctor` follows the same split.** Package-manager steps can only be run as an `Approved` value (typestate). What is wrong and what to install are pure functions in `snapall-core/src/protecting/doctor/`, tested against real `os-release` samples; only `snapall/src/doctor.rs` reads files and runs programs. A new distribution is a new sample and a new test first. Never plan a command that could damage the system (no bare `pacman -Sy`). A family's install plan may run commands as root only if a container in CI installs on that family for real (`only_families_that_ci_installs_for_real_run_commands_as_root` pins it); every other family prints what to run.
13. **Packages come from one script.** `packaging/build-packages.sh` is used by CI and by the release; the container tests in `packaging/test/` must pass for any change to packaging, and `release-verify.yml` checks every published release (run it by hand for a tag with `workflow_dispatch`). Both packages must keep depending on snapper.
14. **Licensing is checked, not remembered.** The project is GPL-3.0-or-later. Every `.rs`, `.sh` and `.py` file needs its SPDX header (`reuse annotate`, see CONTRIBUTING); `reuse lint` and `cargo deny check licenses` run in CI. Never add a dependency whose license is not GPL-3.0-compatible.
15. **One logical change per PR; the PR title is a Conventional Commit** (see CONTRIBUTING.md).
16. **Record decisions.** A significant choice, including what was considered and rejected, gets an ADR in `docs/adr/`. Every known gap goes in `docs/backlog.md`, not in a prose TODO.
