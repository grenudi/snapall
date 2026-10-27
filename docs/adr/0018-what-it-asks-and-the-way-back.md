<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# ADR 18: What snapall asks before it acts, and the way back from what it did

Status: accepted. Built in the command line, the record and the window; not built: an undo key in the window.

## Context

The request was: every change should need `y`/`yes`, destructive ones something more deliberate (`destroy`),
and every logged action should have a rollback that shows the state it returns to, or advice to use snapshots.
An audit first: `snapall delete` asked nothing at all; `setup` took a bare Enter as yes; limits and groups
changed with no question; the window confirmed mostly with Enter.

## Decisions

**Three weights, one table (`snapall_core::consequence`).** *Light* (reading, taking a snapshot): never asked.
*Changes* (can be put back, but putting back costs time and attention): type `y` or `yes`. *Destroys* (lost for
good): type `destroy`, plus the name where it is done to one thing (`destroy home`). The command line, the window
and the tests read the same table, so they cannot disagree.

**Not "ask for everything", and why.** Taking a snapshot is the safety net; if it costs a ceremony it is skipped,
and a prompt on everything trains the hand to type `y` unread, which weakens exactly the prompts that matter.
Confirmation protects only while it is rare enough to be read.

**The word, and the name.** `destroy` says "I understand"; the name says "and it is this one", which guards
against acting on the wrong row. A bare Enter is never an answer, because it is what a hand does by itself.

**A script has a flag of the same weight.** `--yes` for changes, `--destroy` for deletions: `--yes` is not
accepted for a deletion, so a habitual `--yes` in a script cannot delete anything.

**Every change records its way back, when it is made.** An `Undo` (a sentence that names the earlier state, the
commands, and what doing it weighs) or `Advice` (it cannot be undone, and what protects next time). It is
recorded then, not reconstructed later, because only then is the earlier state known (the safety snapshot a
restore took, the limits before they changed, the group as it was). Undoing the taking of a snapshot is
deleting it, so it weighs `Destroys`.

**Honest about what cannot be undone.** Deleted snapshots and an area that stopped being protected cannot come
back; the record says so and points to what does protect: a snapshot, or one of a group (`create --group`),
taken before the risky step.

**Checked when used.** `snapall undo` re-checks that what it would return to still exists (cleanup may have
removed it) and refuses, saying why, if not. It runs the recorded commands through the same program, so they
are checked and journaled like any other, and answers them at the weight it asked.

## Consequences

* Scripts that deleted without a flag now need `--destroy`; changes that asked nothing (`limits`, `groups`,
  `cancel`) now need `--yes`. This is deliberate, and recorded in the README.
* Changes made before this version have no recorded way back, and `undo` says so.
* Not yet: an undo key in the window's Activity tab. The window shows the way back and the command; doing it
  is `snapall undo` for now.

## How it is kept true

A rule that has to be remembered is broken the first time someone forgets it, so each part is tied to its source:

* **One question in the window** (`model/prompts/confirm.rs`). Every change starts through it, asked at the weight
  `Act::weight()` gives it; a prompt cannot start a change without it. Its answers are the core's
  `Weight::accepts`, the same function the command line uses.
* **One place for every way back** (`snapall_core::rollback`). The command line and the window record the same
  words and the same commands for the same change; `Rollback::words` is how both show it.
* **One declaration per command** (`contract.rs`, the `commands!` macro). The enum, the list of all commands, their
  words and every list of who takes `--json`, logs, takes `--dry-run`, `--yes` or `--destroy` are derived from it.
  A test ties the flags to the policy table: a command that can lose something takes `--destroy`, and one that
  only loses something takes no `--yes`, which could not answer for it.
* **Exhaustive, not catch-all, where a change is described** (`changes.rs`): `describe_change`, `describe_now` and
  `how_it_went` are `#[deny(clippy::wildcard_enum_match_arm)]`, so a new kind of change does not compile until it
  has its words and its result; a test goes through every change the window makes and requires a way back for
  one that worked and none for one that failed.
* **Texts are checked against the contract**: every backticked `snapall …` and `--flag` in the code, README and docs
  must exist, and every command must be in the README. The window's help is tied to its buttons (every key on the
  bar is explained there) and must fit the space a prompt has.
