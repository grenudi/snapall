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

Scopes: `core`, `snapper`, `cli`, `ci`, `deps`, `release`, `docs`, `adr`, `dev`. Scope is encouraged, not required.

## Before opening a PR
`cargo fmt --all`, `cargo clippy --workspace --all-features --all-targets -- -D warnings` and `cargo test --workspace --all-features` must pass. See [AGENTS.md](AGENTS.md) for the rules the architecture depends on.
