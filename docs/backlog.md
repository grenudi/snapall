# Backlog

Every known gap lives here, not in a prose TODO.

## Product
| # | Item | Status |
|---|---|---|
| 1 | Publish to the AUR (`snapall` and `snapall-bin`); needs an AUR account and an SSH key as a secret | open |
| 2 | Host signed apt and rpm repositories (or use the openSUSE Build Service / COPR / a PPA) so `apt install snapall` works; sign the packages | open |
| 3 | Per-distro snapshot hooks (pacman, zypper, dnf, apt) behind the existing port | open |
| 3a | Commit `flake.lock`; submit the package to nixpkgs | open |
| 3c | `doctor`: check the filesystem is Btrfs, and `snapperd`/D-Bus state beyond the error text | open |
| 6 | TUI: a termination signal returns at once, but leaving the worker's thread scope joins it, so a snapper call that hangs forever would delay the exit (systemd would SIGKILL after its timeout). Fix: a detached worker (needs an owned, `'static` backend) or exiting the process after the terminal is restored | open |
| 7 | TUI: show what a snapshot changed (diff/status) and restore a file (`undochange`) | open |
| 8 | More `Observer` adapters behind the existing port: journald/syslog, OpenTelemetry. Log rotation stays the environment's job (logrotate, journald) | open |
| 9 | Confirm snapper's allowed config-name characters; `ConfigName` is currently stricter than necessary | open |
| 10 | Branch protection on `main` requiring the CI checks | open |

## Upstream: argenv
| # | Item | Status |
|---|---|---|
| A | `argenv check` rejected `subcommands` (argenv#21) | **fixed in 0.3.0**; CI now runs `argenv check` |
| B | `lint` ignored subcommand scope for `required` (argenv#22) | **fixed in 0.3.0**; `delete`'s arguments are `required: true` again |
| C | `Type::List` needs a closed `allowed` set, so a runtime-determined list cannot be declared (argenv#25) | open upstream (worked around: `--only` is a string, pinned by a test) |
| D | Misspelled flags/variables were warnings and out-of-place flags were accepted (argenv#26, #30, #31) | **fixed in 0.3.0**; snapall's own checks deleted |
| E | argenv README stale (0.1, no `argenv-resolve`) (argenv#24) | **fixed** |
| F | Migrate argenv to schemars 1.x (argenv#28) | open upstream; no effect on snapall |

## Dependencies
| # | Item | Status |
|---|---|---|
| X | Move to ratatui 0.30 when the MSRV moves to 1.88, and drop the `instability` pin | open |
