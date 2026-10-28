# Backlog

Every known gap lives here, not in a prose TODO.

## Product
| # | Item | Status |
|---|---|---|
| 1 | Publish to the AUR (`snapall`, built from the release PKGBUILD). Needs the maintainer's AUR account and an SSH key; the release already attaches a PKGBUILD with the real version and checksum | open |
| 2 | Signed apt and rpm repositories (or the openSUSE Build Service / COPR / a PPA) so that `apt install snapall` works. Needs a long-lived signing key and hosting, so only on demand | on demand |
| 3 | Submit the package to nixpkgs (the flake and its `flake.lock` are in the repository; update the lock with `nix flake update`) | open |
| 4 | TUI: a termination signal returns at once, but leaving the worker's thread scope joins it, so a snapper call that hangs forever would delay the exit (systemd would SIGKILL after its timeout). Fix: a detached worker (needs an owned, `'static` backend) or exiting the process after the terminal is restored | open |
| 5 | TUI: browse the changed files of a snapshot (restore of a whole config and `snapall changes` exist; picking single paths in the TUI does not) | open |
| 6 | `doctor`: show the boot partition's size against limine-snapper-sync's 4 GiB recommendation (read-only; the commonest practical blocker) | open |
| 7 | Confirm snapper's allowed config-name characters; `AreaName` is currently stricter than necessary | open |

What is deliberately *not* here (package-manager hooks, more log sinks, Btrfs checks, branch protection) is in [ADR 11](adr/0011-scope-what-snapall-is-not.md).

| # | Item | Status |
|---|---|---|
| 1 | Name the bare booleans: 34 `yes`, `destroy` and `dry_run` parameters in the commands are nouns that are not named yet (a run mode, real or dry; how a question was already answered) | open |
| 2 | One layer that owns a change (begin, record, end, the way back), used by both the command line (`commands/change.rs`) and the window (`session/executor.rs`), which each do their own today | open |
| 3 | Outputs as values: a handler returns what is to be said and the surface prints it. Fifteen prints still bypass the presenter | open |
| 4 | The window by purpose: its state is one 26-field struct, its questions are `model/prompts/` and `view/prompts/` pairs and its file screen `model/file.rs` with `view/file.rs`. Compose the state by screen | open |
| 5 | The window's `changes.rs` is over 300 lines (419) | open |
| 6 | `restore_all` and `undo` are over the branching budget, and `check_the_way_back_still_exists` over the plumbing budget | open |
| 7 | One question the cold read cannot answer: "What does a key do in the window?" leads to the wrong module first (`crates/snapall/tests/ratchets.json`). Name or describe the key modules for what a stranger would ask | open |
| 8 | Three tests write a script and run it at once (`binary.rs` and the snapper adapter's tests): the same `text file busy` race the background runner's test had. Give them the same retry when one fails | open |
| 9 | The openSUSE x86_64 container job fails now and then in the runner's own setup, seen twice. `dev/ship.py` re-runs it once; find out why (the image pull, a mirror) or pin it | open |
| 10 | Record each change's weight in the journal (asked at: changed, or lost for good), so the Activity tab can say it | open |

Decided, so not open: `setup` is a conversation that explains and then asks about each part, so it is the one change that begins without a single leave (`begin_asking_along`, kept to one place by a test). Asking once up front would make the wizard worse, and it already takes its facts and its way of talking as inputs, so every branch is tested as a conversation with a scripted person. Revisit only if each of its changes should be journaled separately.

## Upstream: argenv
| # | Item | Status |
|---|---|---|
| C | `Type::List` needs a closed `allowed` set, so a runtime-determined list cannot be declared (argenv#25). Worked around: `--only` is a string, pinned by a test | open upstream |
| F | Migrate argenv to schemars 1.x (argenv#28); no effect on snapall | open upstream |

## Dependencies
| # | Item | Status |
|---|---|---|
| X | Move to ratatui 0.30 when the MSRV moves to 1.88, and drop the `instability` pin | open |
