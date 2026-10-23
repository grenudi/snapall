# 4. Drive snapper through its CLI with `--jsonout`

Status: accepted

## Context
snapper can be driven through its D-Bus API, its C++ library, or its CLI.

## Decision
Run the `snapper` CLI and parse `--jsonout`. The exact command lines and the JSON shape were captured from a real snapper (0.10.6) on a real Btrfs volume, and the same contract test runs against it in CI.

## Considered
D-Bus via a Rust binding: more direct, much more surface to get wrong. Parsing human-readable tables: fragile.

## Consequences
The adapter depends on the CLI's JSON format; the real-snapper CI job is what notices if it changes. `--no-dbus` is exposed for containers and CI.
