<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# ADR 18: What snapall asks before it acts, and the way back from what it did

Status: accepted. The command line and the record are built; the window is being brought to the same rule.

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
* Not yet: the window's typed confirmations and an undo key in the Activity tab; changes made in the window
  record their way back only when they are handed to the background (they run the command line).
