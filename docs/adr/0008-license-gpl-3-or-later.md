# 8. License: GPL-3.0-or-later

Status: accepted. Supersedes the initial MIT OR Apache-2.0 choice made with v0.2.0.

## Context
snapall was first licensed MIT OR Apache-2.0, matching argenv, so that distributions could package it. The author then asked for a license that makes anyone who distributes a modified version share its source with the people they give it to.

## Decision
GPL-3.0-or-later.

- **Copyleft, not a patch pipeline.** The GPL obliges a distributor to pass the source (with their changes) on to recipients under the same license. It cannot oblige anyone to send changes back upstream, and using a modified copy privately obliges nothing. If the aim is to receive patches, that is a contribution-process matter, not a licensing one.
- **"or later".** GNU's recommended form; it lets the project follow a future GPL without asking every contributor. `GPL-3.0-only` is the stricter alternative: change `license` in `Cargo.toml`, the SPDX headers and `PKGBUILD`/flake.
- **Not AGPL.** snapall is a local command-line tool and has no network-service use for the extra clause.
- **Dependencies stay as they are.** Every dependency is MIT, Apache-2.0, Zlib, Unlicense, BSL-1.0 or Unicode-3.0, all GPL-3.0-compatible (Apache-2.0 with GPLv3, not with GPLv2). The combined work, including the static binary, is GPL-3.0-or-later. `cargo deny` enforces the allow-list.
- **argenv stays MIT OR Apache-2.0.** snapall links it; being permissive, it is a fine dependency of a GPL program, and keeps being usable by anything.
- **snapper is not linked.** snapall runs the `snapper` program as a separate process, so snapper's license does not reach snapall.
- **Per-file notices.** SPDX headers on source files and `REUSE.toml` for the rest, checked by `reuse lint`.
- **No CLA.** Contributions are inbound = outbound (see CONTRIBUTING). The author holds the whole copyright today; once others contribute, relicensing would need their consent.

## Consequences
- v0.1.0 and v0.2.0 remain MIT OR Apache-2.0 for those who received them; that cannot be withdrawn. New releases are GPL.
- Binary distribution must carry the notices of the MIT and Apache libraries inside it, so each package ships `THIRD-PARTY-LICENSES.md`, generated from the lockfile.
- Some organisations avoid GPL software. Accepted.
- This is the author's reasoning, not legal advice.
