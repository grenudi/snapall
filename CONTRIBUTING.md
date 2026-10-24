# Contributing

## Pull requests are squash-merged; the PR title is the commit

The PR title is linted against [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): subject`, subject lowercase and imperative. It becomes the commit on `main`, and [release-plz](https://release-plz.dev/) reads it to choose the next version and write the changelog.

| Type | Use for | Version bump (pre-1.0) |
|---|---|---|
| `feat` | New behaviour or capability | minor |
| `fix` | A bug fix | patch |
| `perf` | A performance improvement | patch |
| `refactor` | Restructuring without behaviour change | patch, changelog entry |
| `docs` | Documentation only | none |
| `test` | Tests only | none |
| `build`, `ci`, `chore` | Tooling, workflows, housekeeping | none |
| `revert` | Reverting a previous change | patch |

A breaking change adds `!` (`feat(cli)!: ...`) or a `BREAKING CHANGE:` footer.

Scopes: `observe`, `license`, `core`, `snapper`, `tui`, `cli`, `ci`, `deps`, `release`, `docs`, `adr`, `dev`. Scope is encouraged, not required.

## Before opening a PR
`cargo fmt --all`, `cargo clippy --workspace --all-features --all-targets -- -D warnings` and `cargo test --workspace --all-features` must pass. See [AGENTS.md](AGENTS.md) for the rules the architecture depends on.

## Building and testing

```
cargo fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
```

Packaging, `doctor --install` and the packages are tested for real on every push, on both architectures: each of Debian, Ubuntu, Fedora, openSUSE Tumbleweed and Alpine (x86_64 and aarch64, on native arm runners) and Arch (x86_64; it has no official aarch64 image) runs in a fresh container with no snapper, installs through its own package manager, and is checked afterwards (`packaging/test/`). After every release, `release-verify.yml` repeats that on what was actually *published*: it downloads the assets, checks them against the checksums published beside them, installs the released packages, builds the released PKGBUILD from the tag's source, and asserts the version matches the tag. The same script, `packaging/build-packages.sh`, builds the packages CI tests and the packages a release ships.

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

Pull requests are squash-merged and the **PR title** must be a Conventional Commit (above); it becomes the commit on `main`. [release-plz](https://release-plz.dev/) keeps a release PR (version bump plus changelog, via git-cliff) up to date. Merging it tags `snapall-vX.Y.Z`, creates the GitHub release and attaches static `x86_64` and `aarch64` Linux binaries, `.deb` and `.rpm` packages, checksums, and a `PKGBUILD` with the real version and source checksum.

## Licensing of contributions

snapall is GPL-3.0-or-later. By submitting a contribution you license it under the same terms (inbound = outbound); there is no CLA. Every source file carries an SPDX header, and CI runs `reuse lint`: add one to any new file with

```
reuse annotate --copyright "Your Name" --license GPL-3.0-or-later --skip-unrecognised path/to/file
```

Other files (documentation, manifests, workflows) are covered by `REUSE.toml`. Dependencies must be GPL-3.0-compatible; `cargo deny check licenses` enforces it.
