# 2. release-plz, not release-please

Status: accepted

## Context
The canon repository uses release-please, which is language-agnostic and fits a data repo. snapall is a Rust workspace, and `argenv` (same author) already uses release-plz with git-cliff.

## Decision
Use release-plz. It understands Cargo workspaces, keeps `Cargo.lock` consistent with the version bump, and reads the same Conventional Commits. `publish = false` for now: snapall ships as binaries and distro packages, not through crates.io. One GitHub release is created, for the `snapall` binary crate only.

## Considered
release-please with `release-type: rust`: workable for a single package, awkward for a multi-crate workspace.

## Consequences
Only squash-merged PR titles are linted, because that is what lands on `main` and what release-plz reads. Release binaries are built from the same workflow, because events created with `GITHUB_TOKEN` do not trigger other workflows.
