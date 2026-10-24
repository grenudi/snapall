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
| Alpine, Void, Gentoo, anything else | the static `snapall-v<version>-<arch>-unknown-linux-musl.tar.gz` (extract it, then `sudo ./snapall doctor --install`); it also holds the completions and the man page |

`snapall doctor --install` installs the one package snapall needs through the system's own package manager (`apt-get`, `dnf`, `pacman`, `zypper`, `apk`), showing the commands and asking first. Those are the systems it runs commands on because those are the systems CI installs on, for real, on every push. Everywhere else it prints what to run instead of running it: RHEL and its rebuilds (snapper is only in EPEL), Alpine releases other than `edge` (snapper is only in `edge`'s `testing`), Void and Gentoo (`xbps-install -S snapper`, `emerge --ask app-backup/snapper`), NixOS (declarative), and Fedora Atomic, MicroOS and SteamOS (read-only root).

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
| `128 + n` | `tui` was stopped by signal `n` (143 for SIGTERM); the terminal is restored first |
| `2` | the invocation is wrong (unknown flag, a flag that does not apply to the command, a bad value) |

A flag given to a command it does not belong to is **rejected**, not ignored, so `snapall delete home 3 --dry-run` can never delete for real while you believe it is a preview. A one-word typo of a command (`snapall lst`) is refused rather than becoming a snapshot named "lst"; write `snapall create lst` if you mean it.

### Environment

One setting, two doors: each of these is the same input as its flag, and an explicit flag wins.

| Variable | Same as |
|---|---|
| `SNAPALL_ONLY=root,home` | `--only root,home` |
| `SNAPALL_NO_DBUS=1` | `--no-dbus` |
| `SNAPALL_JSON=1` | `--json` |
| `SNAPALL_LOG=text\|json` | `--log text\|json` |
| `SNAPALL_LOG_FILE=/var/log/snapall.jsonl` | `--log-file /var/log/snapall.jsonl` |

A misspelled `SNAPALL_*` variable is an **error** with a suggestion (`SNAPALL_NO_DBU` stops the run and says it meant `SNAPALL_NO_DBUS`), not a setting that is silently not applied.

### Verifying what you downloaded

Every release file carries a signed statement of where it was built (SLSA provenance, Sigstore, no key to trust), and the binary is reproducible: rebuilding the tag gives the same bytes.

```console
$ gh attestation verify snapall-v0.4.2-x86_64-unknown-linux-musl.tar.gz --repo grenudi/snapall
```

How to check the checksums, rebuild a release yourself, and what is verified after every release: [docs/verifying.md](docs/verifying.md).

### Completions and the manual

Every package installs shell completions for **bash, zsh, fish and nushell** and the `snapall(1)` manual page, and the tarball carries them in `completions/` and `man/`. They are not written by hand: the contract that parses the command line also generates them (`snapall completions bash|zsh|fish|nushell`, `snapall man`), so they cannot disagree with what snapall accepts. A flag that belongs to one command, like `--dry-run` for `create`, is offered only after that command, which is the same rule `snapall delete --dry-run` is refused by.

```console
$ snapall completions fish > ~/.config/fish/completions/snapall.fish     # without a package
$ snapall man | gzip -9n > ~/.local/share/man/man1/snapall.1.gz
```

### Logging and the audit trail

snapall is quiet by default. Ask, and every operation it performs (listing, creating, deleting, and each package-manager step of `doctor --install`) produces one line with its outcome and duration:

```console
$ sudo snapall --log text create "before theming"
2026-10-23T08:30:14Z INFO  configs_listed count=2 took_ms=6
2026-10-23T08:30:14Z INFO  snapshot_created config=home snapshot=14 description="before theming" took_ms=29
2026-10-23T08:30:14Z INFO  snapshot_created config=root snapshot=212 description="before theming" took_ms=9

$ sudo snapall --log-file /var/log/snapall.jsonl delete home 14      # an audit trail: appended, mode 0600
$ jq -c 'select(.level == "error")' /var/log/snapall.jsonl
```

`--log text|json` writes to stderr: an event stream you route with your environment (a pipe, journald under systemd). `--log-file` appends JSON Lines to a private file instead, which is what you want from cron or a systemd timer. The `tui` owns the terminal, so it logs only to a file. If a log line cannot be written, the operation still happens: losing a log line is better than failing a snapshot.

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

Snapper calls run in the background: the header shows *working* while one is in flight, the screen stays responsive, and keys that would start more work are refused until it finishes. Quitting while work is in flight waits for it to complete.

> Snapshots live on the same disk. They are not backups.

## Status

Early. Scope today is **snapper-backed snapshots**, which in practice means Btrfs. Repository hosting and signing for the packages, and a few upstream argenv items are tracked in [docs/backlog.md](docs/backlog.md). Licensed under the GNU General Public License, version 3 or (at your option) any later version.

## Architecture

Hexagonal, with a functional core and an imperative shell, enforced by crate boundaries and a fitness test. See [ARCHITECTURE.md](ARCHITECTURE.md), which maps each decision back to the [software-engineering-canon](https://github.com/creation-guidelines/software-engineering-canon), and the [ADRs](docs/adr/).

## Contributing

Build, test and release instructions, the commit convention, and how the CI proves the packages are in [CONTRIBUTING.md](CONTRIBUTING.md); the rules the architecture depends on are in [AGENTS.md](AGENTS.md).

## License

snapall is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It comes with **no warranty**. See [docs/adr/0008-license-gpl-3-or-later.md](docs/adr/0008-license-gpl-3-or-later.md) for why, and `THIRD-PARTY-LICENSES.md` (in every package and release) for the licenses of the libraries it contains.

Releases up to and including v0.2.0 were published under MIT OR Apache-2.0; recipients of those versions keep those rights. Versions after v0.2.0 are GPL-3.0-or-later.
