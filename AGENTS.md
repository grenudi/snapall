# AGENTS.md - how to work in this repository

snapall snapshots every snapper config. The architecture is settled and written down; **work on the behaviour, do not re-derive the structure.** Read [ARCHITECTURE.md](ARCHITECTURE.md) first.

## Commands
```
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features        # includes the architecture fitness test
```
Real snapper (root, loop devices, `snapper`, `btrfs-progs`): see the README's Development section.

## Rules
1. **The dependency rule is a test, not a convention.** `snapall-core` has no I/O and depends only on `thiserror`. No library crate may depend on `clap`. If `crates/snapall/tests/architecture.rs` fails, fix the design, not the test.
2. **Parse at the boundary.** Untrusted text becomes `ConfigName`, `Description` or `SnapshotId` once, in the CLI or in an adapter. Core code never takes a raw `&str` for these.
3. **No shell.** Commands are built as argument vectors and run without a shell. A value that looks like an option must stay one argument.
4. **Total functions.** No `unwrap`/`expect`/panics on anything that comes from the user or from snapper. Add an explicit error variant instead.
5. **Test at the seam where the behaviour lives.** Planning and use-case logic: in-memory backend. Command lines and output parsing: scripted runner. Whether snapper really accepts a command: the contract test against real snapper. Do not guess snapper's output format; capture it from the real tool.
6. **A new backend must pass `snapall_core::testing::backend_contract`.**
7. **One logical change per PR; the PR title is a Conventional Commit** (see CONTRIBUTING.md).
8. **Record decisions.** A significant choice, including what was considered and rejected, gets an ADR in `docs/adr/`. Every known gap goes in `docs/backlog.md`, not in a prose TODO.
