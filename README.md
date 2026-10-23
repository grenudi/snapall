# snapall

Snapshot **every** [snapper](http://snapper.io/) config in one command (root, home, whatever you have configured), or browse, create and delete snapshots in a terminal UI.

```console
$ sudo snapall "before theming"            # shorthand for `snapall create "before theming"`
ok    home  #14
ok    root  #212

$ snapall list
home
root

$ snapall snapshots home
home
   #  type    date                 cleanup  description
  13  single  2026-10-22 18:04:11  number   after update
  14  single  2026-10-23 01:26:30  number   before theming

$ sudo snapall tui
```

## Install

snapall drives [snapper](http://snapper.io/), so every package below **depends on it**: installing snapall installs snapper too. Packages are attached to each [release](https://github.com/grenudi/snapall/releases) (they are not signed yet).

| System | How |
|---|---|
| Debian, Ubuntu, Mint, ... | `sudo apt install ./snapall_<version>_<arch>.deb` |
| Fedora | `sudo dnf install ./snapall-<version>.<arch>.rpm` |
| openSUSE | `sudo zypper install --allow-unsigned-rpm ./snapall-<version>.<arch>.rpm` |
| Arch, CachyOS, Manjaro, ... | download `PKGBUILD` from the release, then `makepkg -si` |
| Nix, NixOS | `nix run github:grenudi/snapall` (the flake wraps snapper into the program's `PATH`) |
| Anything else (Alpine, Void, Gentoo, ...) | the static `snapall-<version>-<arch>-unknown-linux-musl.tar.gz`, then `sudo ./snapall doctor --install` |

`snapall doctor --install` installs the one package snapall needs through the system's own package manager (`apt-get`, `dnf`, `pacman`, `zypper`, `apk`, `xbps-install`, `emerge`). Where a command would be wrong it says so instead of running anything: RHEL and its rebuilds only have snapper in EPEL; NixOS is declarative; Fedora Atomic, MicroOS and SteamOS have a read-only root.

## Commands

| Command | What it does |
|---|---|
| `snapall [create] [DESCRIPTION]` | One snapshot of every config (the default). `--only root,home` restricts it, `--dry-run` previews it. |
| `snapall list` | The configs snapall can see, i.e. exactly what `create` would touch. |
| `snapall snapshots [CONFIG]` | The snapshots of one config, or of all. |
| `snapall delete CONFIG ID` | Delete one snapshot. `0` is the current system and is refused. |
| `snapall doctor [--install]` | Checks that snapper works; with `--install`, installs it with this system's package manager (shows the commands, asks first, needs root). |
| `snapall tui` | The interactive terminal UI. |
| `snapall contract` | This program's invocation contract, as [argenv](https://github.com/argenv-opencommons/argenv) JSON. |

`--json` (on `create`, `list`, `snapshots`) prints machine-readable output; `--no-dbus` talks to libsnapper directly (root only; for containers and CI). Every flag has a documented home in `snapall --help`, which is generated from the same declarations that parse the command line.

Nothing is assumed: an unknown config name is an error, never a silent skip; one failing config does not stop the others, every outcome is printed, and the exit status says whether any failed.

| Exit status | Meaning |
|---|---|
| `0` | done |
| `1` | a snapshot or snapper call failed (or there is no terminal for `tui`) |
| `2` | the invocation is wrong (unknown flag, a flag that does not apply to the command, a bad value) |

A flag given to a command it does not belong to is **rejected**, not ignored, so `snapall delete home 3 --dry-run` can never delete for real while you believe it is a preview. A one-word typo of a command (`snapall lst`) is refused rather than becoming a snapshot named "lst"; write `snapall create lst` if you mean it.

### Environment

One setting, two doors: each of these is the same input as its flag, and an explicit flag wins.

| Variable | Same as |
|---|---|
| `SNAPALL_ONLY=root,home` | `--only root,home` |
| `SNAPALL_NO_DBUS=1` | `--no-dbus` |
| `SNAPALL_JSON=1` | `--json` |

A misspelled `SNAPALL_*` variable produces a warning with a suggestion instead of being ignored in silence.

### The terminal UI

Configs on the left, the selected config's snapshots on the right, newest first.

| Key | Action |
|---|---|
| `j` `k` / arrows | move in the focused pane |
| `h` `l` / `Tab` | switch pane |
| `space` | mark a snapshot, then move on |
| `c` | create a snapshot of the selected config (asks for a description) |
| `a` | create a snapshot of every config |
| `d` | delete the marked snapshots, or the one under the cursor (asks to confirm) |
| `r` | reload from snapper |
| `?` / `q` | help / quit |

> Snapshots live on the same disk. They are not backups.

## Status

Early. Scope today is **snapper-backed snapshots**, which in practice means Btrfs. Shell completions and man pages (both derivable from the contract), repository hosting and signing for the packages, and a few upstream argenv items are tracked in [docs/backlog.md](docs/backlog.md). Licensed under MIT OR Apache-2.0, at your option.

## Architecture

Hexagonal, with a functional core and an imperative shell, enforced by crate boundaries and a fitness test. See [ARCHITECTURE.md](ARCHITECTURE.md), which maps each decision back to the [software-engineering-canon](https://github.com/creation-guidelines/software-engineering-canon), and the [ADRs](docs/adr/).

| Crate | Role |
|---|---|
| `snapall-core` | Domain types, the planner, the port, the use cases. Pure: no I/O, no parser. |
| `snapall-snapper` | Driven adapter: the port, implemented by running `snapper`. |
| `snapall-tui` | Driving adapter: the terminal UI as a pure `update`/`view` state machine. |
| `snapall` | Composition root: the argenv-declared CLI, wiring, rendering, exit codes. |

## Development

```
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
```

Packaging, `doctor --install` and the packages are tested for real on every push: each of Debian, Ubuntu, Fedora, openSUSE Tumbleweed, Arch and Alpine runs in a fresh container with no snapper, installs through its own package manager, and is checked afterwards (`packaging/test/`). The same script, `packaging/build-packages.sh`, builds the packages CI tests and the packages a release ships.

The real-snapper checks need root, loop devices, `snapper` and `btrfs-progs`:

```
sudo ./dev/btrfs-fixture.sh up
sudo python3 dev/tui-e2e.py target/debug/snapall      # the real TUI in a pty (pip install pyte)
SNAPALL_IT=1 cargo test -p snapall-snapper --test real_snapper
sudo ./dev/btrfs-fixture.sh down
```

CI runs all of it on every push, plus `argenv lint` over the committed contract (`snapall.argenv.json`). After changing the CLI surface in `crates/snapall/src/contract.rs`, regenerate it:

```
cargo run -q -p snapall -- contract > snapall.argenv.json
```

## Releases

Pull requests are squash-merged and the **PR title** must be a [Conventional Commit](CONTRIBUTING.md); it becomes the commit on `main`. [release-plz](https://release-plz.dev/) keeps a release PR (version bump plus changelog, via git-cliff) up to date. Merging it tags `snapall-vX.Y.Z`, creates the GitHub release and attaches static `x86_64` and `aarch64` Linux binaries, `.deb` and `.rpm` packages, checksums, and a `PKGBUILD` with the real version and source checksum.
