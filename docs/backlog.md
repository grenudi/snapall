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

## Structure
Where the code still does not say what snapall is for. Each iteration is a branch and a pull request, and none depends on anyone remembering the last. To work on one: read `OUTLINE.md` as a stranger would; run `dev/golden-output.sh check` before and after, because behaviour must not change; split a module by purpose with `dev/split-module.py`; refresh the outline and record paid debts with `UPDATE_OUTLINE=1` and `UPDATE_RATCHETS=1` on the architecture test. The debts themselves are data in `crates/snapall/tests/ratchets.json`, which can only be lowered.

| # | Item | Status |
|---|---|---|
| 1 | The tabs are three folders named as the tab bar names them (`tabs/snapshots/`, `tabs/health/`, `tabs/activity/`). What is left by mechanism is the questions: `model/prompts/` and `view/prompts/` hold one pair of files per question, and `model/file.rs` with `view/file.rs` the file history screen. Also only Health and Activity own their state; the snapshots tab's still lives in the window's `State` | open |
| 2 | `setup.rs` (432 lines) asks area by area as it works, which is why it is the one change that begins without a leave (`begin_asking_along`). Plan first, ask once, then begin | open |
| 3 | `restore_all` and `undo` are over the branching budget, and `check_the_way_back_still_exists` over the plumbing budget | open |
| 4 | The window's `changes.rs` is over 300 lines (419) | open |
| 5 | Code that wraps tools but lives in the binary (`doctor.rs`, `boot_probe.rs`) still names them (`systemctl`, `grub_btrfs`, `snapper`). It belongs in an adapter | open |
| 6 | Ten plain `config` names remain in the binary, in files that already used `area` for something else | open |
| 7 | Run `dev/golden-output.sh check` in CI. It needs its own fresh fixture, after the steps that change the disk | open |
| 8 | Three tests write a script and run it at once (`binary.rs` and the snapper adapter's tests): the same `text file busy` race the background runner's test had. Give them the same retry when one fails | open |
| 9 | Record each change's weight in the journal (asked at: changed, or lost for good), so the Activity tab can say it | open |

## Upstream: argenv
| # | Item | Status |
|---|---|---|
| C | `Type::List` needs a closed `allowed` set, so a runtime-determined list cannot be declared (argenv#25). Worked around: `--only` is a string, pinned by a test | open upstream |
| F | Migrate argenv to schemars 1.x (argenv#28); no effect on snapall | open upstream |

## Dependencies
| # | Item | Status |
|---|---|---|
| X | Move to ratatui 0.30 when the MSRV moves to 1.88, and drop the `instability` pin | open |
