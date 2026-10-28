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

## Upstream: argenv
| # | Item | Status |
|---|---|---|
| C | `Type::List` needs a closed `allowed` set, so a runtime-determined list cannot be declared (argenv#25). Worked around: `--only` is a string, pinned by a test | open upstream |
| F | Migrate argenv to schemars 1.x (argenv#28); no effect on snapall | open upstream |

## Dependencies
| # | Item | Status |
|---|---|---|
| X | Move to ratatui 0.30 when the MSRV moves to 1.88, and drop the `instability` pin | open |
