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

## Working in loops: how an iteration goes
Each iteration is one coherent step with its own branch and pull request, and none depends on anyone remembering the last. State lives in the repository and never in a session: the plan is `docs/backlog.md` (the Structure section), the history is `docs/development-log.md`, the debts are `crates/snapall/tests/ratchets.json`, the outline is `OUTLINE.md`. `/tmp` is not memory.

1. **Read** the plan and the last entries of the log. Read `OUTLINE.md` as a stranger would. The structure trend is the summary of the latest `Structure` workflow run on main (`.github/workflows/structure.yml`), or `dev/structure-metrics.py --trend`.
2. **Choose by evidence, not by order.** Measure (grep, the metrics, the findability test), read the code as it is, and be willing to reverse the plan: `setup` was decided against its own backlog item.
3. **Branch**: `git checkout -b refactor/what-it-is`. **Change** with the tools below. Update the plan and the log in the same change; every path a document names must exist (a test checks).
4. **Gate**: `dev/gate.sh`, about a minute. The heavy checks run in CI on every pull request; reproduce a CI failure with `dev/gate.sh --full`.
5. **Ship**: `nohup env GITHUB_TOKEN=... dev/ship.py "refactor: lowercase subject" --body-file /tmp/body.md > /tmp/ship.log 2>&1 &`, then read `tail /tmp/ship.log`. It pushes, opens the pull request, waits for CI, squash-merges, waits for main, merges the release pull request, and verifies the release. It keeps no state, so run it again after any interruption.
6. **Log** what was done, why, and the evidence, in `docs/development-log.md`.

## The tools, all in `dev/`
| | |
|---|---|
| `gate.sh` | the fast gate (format, clippy, docs, links, licences, tests that need no disk); `--full` adds the fixture checks |
| `ship.py` | push, pull request, CI, merge, release, verify; resumable; re-runs a job the runner itself failed to start |
| `split-module.py` | split one module into a folder of modules by purpose, by a JSON spec, moving whole items |
| `golden-output.sh` | `record` or `check` what 20 commands print, so a refactor is shown to change nothing |
| `structure-metrics.py` | the proxies for "every layer is the top of the layers beneath it", at any tag |

On the architecture test: `UPDATE_OUTLINE=1` regenerates `OUTLINE.md`; `UPDATE_RATCHETS=1` lowers or removes paid debts and never raises one; `FINDABILITY_REPORT=1 cargo test -p snapall --test findability -- --nocapture` prints the ranking for each question. The findability test is the cold read: the outline alone must lead a stranger's question to the right module (`crates/snapall/tests/findability.json`). To add a question, add it there.

## Traps already fallen into
- The tool shell is `sh`: use `bash -c` for `<(...)`, `time` and arrays, or the whole line fails to parse and none of it runs.
- `cargo fmt` reflows source, so a script that patches source by string must assert that its replacement applied; a mutation that silently did not apply proves nothing. Prove a test bites by planting the violation, seeing it fail, and restoring the file with `cmp`.
- A `crate::` path belongs to the crate it is written in; a rewrite across the workspace must not touch another crate's own modules.
- The release commit is the commit titled `chore: release`, not the merge commit before it, whose release workflows are skipped.
- The openSUSE x86_64 container job sometimes fails in the runner's own setup (the step `A user's tools`), before any of our code runs: re-run only that job once. `ship.py` does.
- Size limits get gamed: files huddle just under them. Judge by the findability test and by reading, not only by the numbers.
- A measurement that reads zero before a file became a folder is an artifact of the move, not a result.
