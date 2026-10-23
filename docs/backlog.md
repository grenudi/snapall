# Backlog

Every known gap lives here, not in a prose TODO.

## Product
| # | Item | Status |
|---|---|---|
| 1 | Choose a license (`argenv` uses MIT OR Apache-2.0) | open |
| 2 | Distro packages: `.deb`, `.rpm`, AUR `PKGBUILD`, a Nix flake; consider the openSUSE Build Service for multi-distro builds | open |
| 3 | Per-distro backends and hooks (pacman, zypper, dnf, apt) behind the existing port | open |
| 4 | Shell completions and a man page, generated from the contract (argenv names both as planned generators) | open |
| 5 | `snapall doctor`: report missing tools, show (and with confirmation run) the install command for the detected distro | open |
| 6 | TUI: run snapper calls off the UI thread (a long delete currently freezes the screen until it finishes) | open |
| 7 | TUI: show what a snapshot changed (diff/status) and restore a file (`undochange`) | open |
| 8 | Observability as a port (structured logs), once there is something to observe | open |
| 9 | Confirm snapper's allowed config-name characters; `ConfigName` is currently stricter than necessary | open |
| 10 | Branch protection on `main` requiring the CI checks | open |

## Upstream: argenv (not changed from here)
| # | Item | Status |
|---|---|---|
| A | `argenv check` rejects `subcommands`: `KNOWN_FIELDS` in `crates/argenv-cli/src/main.rs` omits it. Adding the line makes snapall's contract pass and argenv-cli's tests still pass. Then add `argenv check` to the CI `contract` job | open |
| B | `lint` ignores subcommand scope for `required` (`check_resolution` in `lint.rs`) | open (worked around, pinned by a test) |
| C | `Type::List` needs a non-empty `allowed`, so a runtime-determined list cannot be declared | open (worked around, pinned by a test) |
| D | `UnknownArg` is only a warning; flags are parsed globally regardless of subcommand scope | open (worked around, pinned by a test) |
| E | argenv README still says `argenv = "0.1"` and omits the `argenv-resolve` crate; the crate is 0.2.0 | open |

## Dependencies
| # | Item | Status |
|---|---|---|
| X | Move to ratatui 0.30 when the MSRV moves to 1.88, and drop the `instability` pin | open |
