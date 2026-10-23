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

Scopes: `license`, `core`, `snapper`, `tui`, `cli`, `ci`, `deps`, `release`, `docs`, `adr`, `dev`. Scope is encouraged, not required.

## Before opening a PR
`cargo fmt --all`, `cargo clippy --workspace --all-features --all-targets -- -D warnings` and `cargo test --workspace --all-features` must pass. See [AGENTS.md](AGENTS.md) for the rules the architecture depends on.

## Licensing of contributions

snapall is GPL-3.0-or-later. By submitting a contribution you license it under the same terms (inbound = outbound); there is no CLA. Every source file carries an SPDX header, and CI runs `reuse lint`: add one to any new file with

```
reuse annotate --copyright "Your Name" --license GPL-3.0-or-later --skip-unrecognised path/to/file
```

Other files (documentation, manifests, workflows) are covered by `REUSE.toml`. Dependencies must be GPL-3.0-compatible; `cargo deny check licenses` enforces it.
