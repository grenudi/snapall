# 1. Hexagonal architecture, boundaries enforced by crates

Status: accepted

## Context
snapall must work with more than one snapshot mechanism over time (other distros, other tools), be driven by more than one front end (CLI now, a TUI later), and be testable without root or a real filesystem.

## Decision
Follow the canon's default: [hexagonal architecture](https://github.com/creation-guidelines/software-engineering-canon/blob/main/docs/architectures.md#hexagonal) with a functional core and imperative shell. The core owns one port (`SnapshotBackend`); the snapper CLI is one adapter; the binary is the composition root. Boundaries are crates, and a test reads `cargo metadata` to fail on a violation.

## Considered
- A single crate with modules: simpler to release, but the boundary would then be a convention, and the canon says documentation alone does not keep boundaries.
- Putting the CLI parser in the core: rejected, it would make a second front end (TUI) depend on the first.

## Consequences
Three crates to version together. release-plz handles that; see ADR 2.
