<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# The idea

snapall is a small program that lets you try risky things on your computer and go back if they go wrong. This page is about how the *code* is meant to feel to read, because that is the thing everything else in this repository has been shaped around.

## Every layer is the top of the layers beneath it

Imagine reading the program from the top. The first thing you meet says what the app is *for*, in the words a person would use: take a snapshot, go back, protect an area. You can stop there and know the app.

Each step down answers one question: *how is that done?* And it answers it using only the words of the step beneath. Going back to a restore point is written as a handful of plain steps (look at what would change, ask, begin, do it, end, show the result). Each of those steps is a word the layer below provides, and that layer in turn is written in words from the one below it, until you reach the places that touch the machine.

So every part of the program has two faces. An **upward face**: what it is for, and the words it offers. A **downward face**: a body that uses nothing but the words beneath it. If a part can't be described that way, it is doing two jobs, and it gets split.

The result is that you never have to hold the whole program in your head. You read a level, understand it, and open the next one only if you want to know *how*.

## One request, from the top to the machine

Ask to put a restore point back (`snapall restore-all <point>`) and follow it:

1. `crates/snapall/src/main.rs` reads what was asked, sets up what it works with, carries it out and ends with the right exit code. A screenful.
2. `crates/snapall/src/commands/asked_for.rs` is a table of everything that can be asked for, and which command does it. No logic, just names.
3. `crates/snapall/src/commands/going_back/restoring_a_point.rs` is the command, and it reads as the story it is. Look, ask, begin, do it, end, show.
4. `crates/snapall-core/src/going_back/points/putting_back.rs` is how a restore point is put back, area by area, and what each area's outcome means. It knows nothing about disks or terminals.
5. `crates/snapall-core/src/ports/machine/probe.rs` is the one place the core says what it needs to ask the machine, in its own words. The adapters (`crates/snapall-snapper`, `crates/snapall-system`) answer, and only they know where things live on a Linux system.

The window (`crates/snapall-tui`) is a second way in to the same acts, and uses the same words from the same core, so what you read in a terminal and what the window says are one sentence, written once.

## How you can tell it is healthy

These are not rules to memorise. They are checks the repository runs on itself, each one the idea made concrete.

- **A stranger can find things.** `OUTLINE.md` is generated from what each part says it is for, and a test asks it real questions ("where is it said which kind of disk snapall works with?") and fails if the answer is not near the top. If a part's description is vague, it shows up here first.
- **One thing has one word.** A person-readable sentence is written once, in the core, and the guards in `crates/snapall/tests/architecture.rs` stop a plural, a rule, or a stretch of code from being copied into a second place.
- **No tool talk.** The words of the tools underneath (snapper, config, subvolume) stay in the adapters that drive them. Above them, an area is an area.
- **Parts are small enough to take in.** A file is one thing short enough to read whole; a command reads as a handful of named steps, not a page of plumbing.
- **A mistake in the structure fails loudly.** Each of these checks has been shown to fail when the thing it guards is broken on purpose.

## How close are we?

Honestly: close in the middle, less close at the edges.

**Closest:** the commands. Every command is a short story, the request table reads as plain English, and the words a person reads (counts, plurals, error messages, what a restore point is) each live in one place. The core is folders named for what snapall is *for*, not for how it is built.

**Close:** the window. It has been split into screens and questions that each own their state and their drawing, and it speaks through the same words as the command line.

**Close too, which surprised me:** the adapters. Every one says what it is for in its first line ("Talking to snapper: our snapshot verbs, said to it one command at a time"), and their functions are short. The table of every flag (`crates/snapall/src/contract/inputs.rs`) is a table, so it reads as one, but its sections now say what they are for.

**Least close, and said plainly:** the drawing code, which is long because drawing is. And the words of the tools themselves (snapper, `config`) still appear in a few places that other programs read, such as the published contract and the audit log, where renaming them would break those programs.

The measure we use is simple: someone who has never seen the code should be able to say what a part is for from its first line, and be right.

## Where to go next

- `ARCHITECTURE.md`: which design principles snapall applies, and where.
- `docs/canon/small-languages.md`: the idea above, worked through on one real command, with the code it shows kept honest by a test.
- `OUTLINE.md`: the whole program as a table of contents.
- `docs/development-log.md`: how it got here, including the mistakes.
