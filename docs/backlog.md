# Backlog

Every known gap lives here, not in a prose TODO.

## Product
| # | Item | Status |
|---|---|---|
| 1 | Publish to the AUR (`snapall`, built from the release PKGBUILD). Needs the maintainer's AUR account and an SSH key; the release already attaches a PKGBUILD with the real version and checksum | open |
| 2 | Signed apt and rpm repositories (or the openSUSE Build Service / COPR / a PPA) so that `apt install snapall` works. Needs a long-lived signing key and hosting, so only on demand | on demand |
| 3 | Commit `flake.lock`; submit the package to nixpkgs | open |
| 4 | TUI: a termination signal returns at once, but leaving the worker's thread scope joins it, so a snapper call that hangs forever would delay the exit (systemd would SIGKILL after its timeout). Fix: a detached worker (needs an owned, `'static` backend) or exiting the process after the terminal is restored | open |
| 5 | TUI: show what a snapshot changed (diff/status) and restore a file (`undochange`) | open |
| 6 | Confirm snapper's allowed config-name characters; `ConfigName` is currently stricter than necessary | open |

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
