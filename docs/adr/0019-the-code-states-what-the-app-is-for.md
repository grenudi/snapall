<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# ADR 19: The code states what the app is for, in layers, and nothing restates it

Status: accepted. The rule and the first layer are built; the layer that owns a change is designed, not built.

## Context

The request: the code should read almost like prose that fully says what the app does and how, from its whole
purpose at the top to details below, *in code, not in comments*, so that it cannot drift the way prose does.

Measured honestly first, the code did not read that way:

* `commands.rs` was 1,303 lines of handlers that each mix asking, doing, recording and printing; six ran from 63
  to 107 lines. Reading them does not tell you what `snapall` is for.
* The same orchestration (a change starts, is held against another, is performed, records how to get back, ends)
  was written twice: in the command line's handlers and in the window's executor.
* The facts "what this does, what it weighs, what it is called, which flags answer it" were written in four
  places and held together by tests. A test shows two copies agree; it does not remove the second copy.

## The rule (for the canon)

1. **The top layer is executed, never merely described.** A statement of purpose that is not what runs, or what
   the running code is derived from, is a comment with more ceremony: it drifts for the same reason. If a layer
   can be deleted and the program is unchanged, it was prose.
2. **Layers by the stepdown rule.** Each layer is written in the words of the layer above it and calls or derives
   from the layer below; none reaches past its neighbour. For snapall: *purpose* (what a person comes for) ->
   *acts* (what the app does, each with a name, a weight and a way back) -> *changes* (an act made, with its
   result) -> *steps* (the ports: snapper, the system, the journal).
3. **Derive, do not verify, wherever possible.** Where a fact must be known in several places, declare it once
   and compute the rest. Write a test only where derivation is impossible (two crates that must not depend on
   each other), and then the test is written against the one declaration, never against a second hand copy.
4. **Make the wrong state unrepresentable.** Where a new kind of thing must be decided about, match exhaustively
   (no catch-all arm; lint it with `clippy::wildcard_enum_match_arm`), so the compiler asks. Where an order must
   be kept (a change must be approved before it is made), carry it in the type, so the wrong order does not compile.
5. **A function reads as a sentence.** Past a stated length it is doing the work of several and is split by what
   it does. Where existing code breaks this, a ratchet records the debt: no new offender, no growth, and an entry
   leaves the list the moment it is short enough.
6. **The top layer is small enough to read in one sitting**, or it is not a top layer.

## What is not claimed

* Not everything reads as prose. An algorithm is an algorithm; a layer of names that only rename the layer below
  is indirection, not clarity. A layer earns its place by being *used*: by the contract, the window, the tests.
* Layering costs navigation. The rule is worth it where the same fact would otherwise be written twice; it is
  not a licence to wrap every function.
* It does not replace tests. It shrinks what they have to hold together.

## Built

* **`Act` is the app's vocabulary** (`snapall_core::vocabulary::consequence`): every act listed once (`Act::EVERY`), with its
  weight and its title, both exhaustive, so a new act does not compile until it has both.
* **The contract is written in it.** Each command declares what it *does* (`.does(Act::DeleteSnapshots)`); whether
  it takes `--yes` or `--destroy` is derived from the weight of those acts, not declared again. The derived
  contract equals the old one for all 36 inputs.
* **The window is derived from it.** The title of each question is `Act::title`, typed nowhere else; a button says
  what it is for (`Button::purpose`, exhaustive) and looks dangerous because what it is for loses something for
  good, not because a list names it.
* **The ratchet** on handler length (`architecture.rs`): 60 lines, six handlers listed at their present length.

## Designed, not built: the layer that owns a change

`snapall_core::change`: a `Change` enum (take, delete, restore, restore a point, protect, stop protecting, set
limits, switch room counting, save groups, ...). Each variant has its act, its words in both tenses, how it is
performed, and its way back, in **one exhaustive place**, which replaces the window's `describe_change`,
`describe_now`, `how_it_went`, `way_back` and background-arguments functions and the command line's repeated
begin/rollback/end. `make(change, approved)` takes an `Approved`, a value only the answer to the question (or an
act that asks nothing) can produce, so a change cannot be made unasked: the rule that is today kept by each handler
calling the gate becomes a type error. The handlers then become what they should be, a sentence each (parse,
ask, make, say), and the six on the ratchet leave it.

Done when: the ratchet list is empty; the window's executor has one function where it has six; and a new kind of
change is one variant plus the compiler's list of what it still owes.
