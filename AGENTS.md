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
1. **The dependency rule is a test, not a convention.** `snapall-core` has no I/O and depends only on `thiserror`. The TUI, the snapper adapter and the binary never depend on each other except through the binary's wiring. Only the binary knows argenv. If `crates/snapall/tests/architecture.rs` fails, fix the design, not the test.
2. **The CLI surface has one home: `crates/snapall/src/contract.rs`.** Flags, positionals, environment variables and which command each applies to are declared there with argenv; `--help`, parsing and `snapall contract` all derive from it. After changing it, regenerate `snapall.argenv.json` (`cargo run -q -p snapall -- contract > snapall.argenv.json`); a test fails if it is stale.
3. **Parse at the boundary.** argv, environment and snapper output become `ConfigName`, `Description` or `SnapshotId` once, in `cli.rs` or an adapter. Core code never takes a raw `&str` for these.
4. **No shell, no unvalidated argv.** Commands are argument vectors run without a shell. A value that looks like an option stays one argument.
5. **Total functions.** No `unwrap`/`expect`/panics on anything that comes from the user or from snapper. Add an explicit error variant instead. Write to stdout through `emit` in `main.rs`, never `println!`: a closed pipe must not panic.
6. **The TUI's behaviour lives in `snapall-tui/src/model.rs`** as the pure `update`. Drawing stays in `view.rs`; effects are run only in `session.rs`. A new key or flow is a test against `Session` first.
7. **Test at the seam where the behaviour lives.** Planning and use cases: in-memory backend. Command lines and output parsing: scripted runner. Invocation rules: `cli::parse(args, env)` with an injected environment. Whether snapper accepts a command: the contract test against real snapper. Do not guess snapper's output format; capture it from the real tool.
8. **A new backend must pass `snapall_core::testing::backend_contract`.**
9. **Third-party behaviour we work around is pinned by a characterization test** (see `cli.rs`). If one fails after an upgrade, remove the workaround.
10. **MSRV is 1.85.** `Cargo.lock` is committed and pins `instability` to 0.3.10 (see ADR 6). Do not `cargo update` past it, and do not bump ratatui without moving the MSRV deliberately.
11. **`doctor` follows the same split.** What is wrong and what to install are pure functions in `snapall-core/src/doctor.rs`, tested against real `os-release` samples; only `snapall/src/doctor.rs` reads files and runs programs. A new distribution is a new sample and a new test first. Never plan a command that could damage the system (no bare `pacman -Sy`).
12. **Packages come from one script.** `packaging/build-packages.sh` is used by CI and by the release; the container tests in `packaging/test/` must pass for any change to packaging. Both packages must keep depending on snapper.
13. **One logical change per PR; the PR title is a Conventional Commit** (see CONTRIBUTING.md).
14. **Record decisions.** A significant choice, including what was considered and rejected, gets an ADR in `docs/adr/`. Every known gap goes in `docs/backlog.md`, not in a prose TODO.
