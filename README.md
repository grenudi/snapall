# snapall

Take a snapshot of **every** [snapper](http://snapper.io/) config in one command: root, home, and anything else you have configured.

```console
$ sudo snapall "before theming"
ok    home  #14
ok    root  #212

$ sudo snapall create --only home --dry-run
dry run: would snapshot 1 config(s) with description "snapall"
  home

$ snapall list
home
root
```

- `snapall [create] [DESCRIPTION]` snapshots every config snapper reports. Nothing is assumed: `snapall list` shows exactly what it will touch, and `--only a,b` restricts it. An unknown config name is an error, never a silent skip.
- One failing config does not stop the others. Every config gets its attempt, every outcome is printed, and the exit code is non-zero if any failed.
- Snapshots are taken with the `number` cleanup algorithm, so snapper's own cleanup timer prunes them.
- Typically needs root (snapper's own rules apply). `--no-dbus` talks to libsnapper directly, for containers and CI.

> Snapshots live on the same disk. They are not backups.

## Status

Early. Scope today is **snapper-backed snapshots**, which in practice means Btrfs. Distro packaging, a TUI and `argenv`-based argument handling are tracked in [docs/backlog.md](docs/backlog.md). No license has been chosen yet.

## Architecture

Hexagonal, with a functional core and an imperative shell, enforced by crate boundaries and a fitness test. See [ARCHITECTURE.md](ARCHITECTURE.md), which maps each decision back to the [software-engineering-canon](https://github.com/creation-guidelines/software-engineering-canon), and the [ADRs](docs/adr/).

| Crate | Role |
|---|---|
| `snapall-core` | Domain types, the planner, the port, the use case. Pure: no I/O, no parser. |
| `snapall-snapper` | Adapter: the port, implemented by running `snapper`. |
| `snapall` | Composition root: CLI parsing, wiring, rendering, exit codes. |

## Development

```
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
```

The real-snapper contract tests need root, loop devices, `snapper` and `btrfs-progs`:

```
sudo ./dev/btrfs-fixture.sh up
SNAPALL_IT=1 cargo test -p snapall-snapper --test real_snapper
sudo ./dev/btrfs-fixture.sh down
```

CI runs the same thing, plus the real binary end to end, on every push.

## Releases

Pull requests are squash-merged and the **PR title** must be a [Conventional Commit](CONTRIBUTING.md); it becomes the commit on `main`. [release-plz](https://release-plz.dev/) keeps a release PR (version bump plus changelog, via git-cliff) up to date. Merging it tags `snapall-vX.Y.Z`, creates the GitHub release and attaches static `x86_64` and `aarch64` Linux binaries with checksums.
