<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# ADR 20: Every module is a small language, and the outline is generated

Status: accepted. The rules and their ratchets are built; the code is being brought to them, in the order below.

## Context

ADR 19 set the principle (the top layer is executed, not described) and built the first layer. The request that
followed: make *every* layer work that way, each a short front file of screaming states and actions, down to the
point where the language is not ours, and write the rule in full so that a person or an agent can operate by it.

Measured before building (and the measure is the baseline the ratchets hold):

* Output is not in one place: text for a person was printed from seven files besides the presenter.
* Front files were not maps: the snapper adapter's `lib.rs` is 584 lines, the window's `model/mod.rs` 385,
  `main.rs` 196, `tabs/health/mod.rs` 317.
* 20 files exceed 300 lines; the command line's single `commands.rs` was 1,303.
* 63 identifiers above the adapters use the tools' words (`config`, `snapper`, `subvolume`), while the
  vocabulary promised to people is "protected area".
* Every one of the 103 source files already states its purpose, so the outline could be generated at once.

## Decision

The rule is `docs/canon/small-languages.md` (ten rules, a procedure, templates, anti-patterns), written for the
canon. It is enforced by machine wherever it can be, with ratchets: debt written down with its size, nothing
worse, nothing new, entries leave when paid.

## Built

* `OUTLINE.md`, generated from the first sentence of every module's `//!` and checked against the code: the
  table of contents cannot drift. It already caught a false purpose: `output.rs` says it is "the one place output
  leaves the program" while seven files print on their own; the ratchet records them.
* Ratchets: file length (300), front-file length (120), printing outside the presenter and the asker, tool words
  in identifiers above the adapters, handler length (60).
* The pilot: `commands.rs` became `commands/`, one module per purpose (`taking`, `deleting`, `going_back`,
  `protecting`, `keeping`, `looking`, `running`, `window`, and `change` for what they share), with a front file
  that is only the map. Items were moved whole; behaviour is unchanged.

* Story budgets (R11, R14): how often a handler branches (4) and how many closures it spells out (2), as
  ratchets. They name the same handlers already known to be too long and two that were not (`setup`,
  `point_confirmation`), which is some evidence that they measure something real.
* The first story written to the grammar: `delete`, 55 lines to 19, with its words beneath it, a lookup verb in
  the core (`find_snapshot`) and `Change::end_with`. It is worked through in the canon text, with what its
  remaining syntax says the layer still lacks.

## Order of the rest

1. `going_back` (390) and `keeping` (336) into one module per command; handlers return what to say, and `output`
   presents it, which empties the printing ratchet.
2. The window: `model/mod.rs` becomes a map of the update loop's parts; `changes.rs` and `session.rs` give way
   to the layer that owns a change (ADR 19).
3. The adapters' front files: `snapall-snapper/src/lib.rs` as a map of what it translates.
4. The vocabulary: `config` -> `area` through the core, a rename that is mechanical and wide.
5. `contract.rs`, `render.rs`, `cli.rs` split by what each declares, renders or parses.

Each step lowers entries in the ratchets; done when the lists are empty.
