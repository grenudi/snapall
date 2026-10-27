<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Every layer is the top of the layers beneath it

A way of writing software so that the code itself is the specification and cannot drift from it. It is meant to be
followed by a person or by an agent, and it is an idea first and a set of checks second.

## The idea

A program is a stack of layers, and each layer is a small language. It has nouns (the states it can be in, as
types), verbs (the actions it offers, as functions) and a story: how its verbs are made of the verbs of the layer
below.

**Each layer has two faces: an upward one, what it is for and the words it offers the layer above, and a downward
one, a body written only in the words of the layers beneath it. A layer below is itself the top of the ones under
it, so the same shape repeats at every level.**

So reading any one module tells you what it is for and what it offers, without going down. Going down is always
optional, and each step down is another language, read the same way. Where this holds, a person can learn what the
program does by reading it, and nothing has to be written a second time in prose, which is the part that drifts.

It is stratified design (Abelson and Sussman: a system as a sequence of levels, each with the primitives, means of
combination and means of abstraction fit for that level), information hiding, deep modules, ports and adapters,
screaming architecture and a ubiquitous language, held together as one idea instead of five habits.

## What a layer is

*Top* is relative. The command line and the window are the top of the whole program, a small adapter is the top of
the tool it wraps, and every module in between is the top of what it uses. The layers form a graph and not a tree,
because a lower layer serves many above it; the one structural rule is that dependencies point downward.

A module is a directory or a file. What it offers upward is its *front*, which is a role every module plays and not
a kind of file: the entry file of a directory, the public items at the top of a one-file module.

A module speaks one language. One that needs two levels, such as arranging a window and also calling a tool, is two
modules, whatever its length. Length is only a smell that a level is hiding in it: a long file that is one
declarative table is fine, and a short one that mixes two levels is wrong.

## Where it stops

The recursion ends where the language is not ours: the compiler, the standard library, other people's crates, the
operating system, the tools we call. There, each foreign thing is wrapped once, in our words. The wrapper's upward
face uses only our nouns, and nothing above it names the tool. What we have learned about the foreign world is kept
as tests named as sentences (`removing_an_area_deletes_every_snapshot_and_leaves_its_files`) that run against the
real thing, so they document the boundary and cannot drift: if the tool changes, the build says so.

## What the top layers hold

The top layers decide *which*, *when* and *where*, and never *how*. They do contain logic, in the smallest
portions: choosing between named outcomes, putting steps in order, arranging what sits above or beside what, and
streaming inputs into stories and results into the presenter. Each piece of how lives in the lowest layer that can
state it without knowing the layer above. A line at the top then reads as common sense, needs no knowledge of the
language, the libraries, the system or the hardware, and is also the real, running implementation: the idea, in the
flesh of working code.

A story reads as a sequence of verbs on nouns: ask, do, record, say. Lifetimes, format strings, `Option` and
`Result` plumbing, bare booleans and `None` arguments are hidden behind verbs named for what they mean. Whatever
syntax still shows in a story is not to be tolerated but read, because it marks a noun the layer has not named yet.

Arrangement is data at the top. What sits where (a window's tabs above its page above its status above its
buttons) and what can be asked for (the commands and the flags each takes) is a declared tree or table of nouns,
interpreted by a lower layer that knows rectangles, cells and terminals. The top says *above* and *beside*, and the
numbers live below.

Every input a person can give is a value of one enumeration, and every output they can be shown reaches them through
one presenter, so handlers return what is to be shown and do not print it. Then "everything a person can do and see"
is a list that can be read, tested and generated from.

## What a reader gets

Names are the person's words (*snapshot*, *restore point*, *protected area*), never a design pattern's (`Manager`,
`Helper`) and never a tool's (`config`, `subvolume`) above the code that wraps the tool. Module and directory names
then read as the purposes of the program, and the file listing is the first page of the documentation.

Each word is defined before it is used, in the layer the reader is in or the one directly below. The domain is
taught by its types, each noun defined once with its relations: *a restore point is one snapshot in every area,
taken together*.

The test is a **cold read**. Give someone, or an agent, only the generated outline and the entry points, and ask
what the program does and what happens when a snapshot is deleted. Where they cannot say, a name is missing.

## Why it cannot drift

A fact is declared once and everything else is derived from it: the command list from the contract, the flags a
command takes from the weights of the acts it does, a button's look from what it is for, a question's title from
the act. A test that compares two hand-written copies means a copy should go. Tests are for what cannot be derived
(two modules that may not depend on each other), and they are written against the single declaration.

Prose may name, and only code may claim. A module's sentence says what it is *about*, as a noun phrase. A
guarantee (*only*, *never*, *always*) belongs in a test or a type, where it can fail; in prose it can be false while
the build stays green. (One module said it was the one place output leaves the program while seven others printed
by themselves.) The outline of the whole program is generated from those sentences and checked against the code.

The *why* needs a carrier that is checked or generated: names and types (a `Destroys` weight is a reason, an
`Approved` is a rule), the executable facts above, and the generated outline. Comments are for machine-readable
annotations (`TODO(owner)`, a safety note, an attribute). A reason that no carrier holds is a missing name or a
missing test, not a missing comment. This is contested: Ousterhout argues that not everything can be put in code and
that comments should say what is not obvious. It is held here as a discipline and not as a law, and is revisited if
it costs more than the drift it prevents.

Wrong states are made unrepresentable. Where a new kind of thing must be decided about, matches are exhaustive and
the catch-all arm is forbidden, so the compiler asks. Where an order matters, such as a change being approved
before it is made, the order is carried in a type.

## Keeping it honest

Some of this cannot be true of existing code yet, and a *ratchet* keeps it honest meanwhile: the debt is written down
with its size, nothing may get worse, nothing new may join, and an entry must leave the list the moment it is paid.
The measures are alarms that find candidates, and they do not define a good module: files past 300 lines, entry
files past 120, functions past 60, stories that branch more than four times or spell out more than two closures,
printing outside the presenter and the asker, and a tool's words in names above its wrapper.

## Working this way

*Adding a feature.* Say what it is for as an act in the app's vocabulary. Declare it where that vocabulary is
consumed, and the flags, questions and labels follow. Write its story in the module whose purpose it is, in the
verbs of the layer below, and if a verb you need does not exist, write that first, in the layer below, in that
layer's words. If it touches something foreign, the translation goes in the wrapper and never in the story. Run
the gates and regenerate the outline.

*Where does this code go?* Ask which purpose of the app it serves (the module), then which layer: it names a tool
(the wrapper), decides what to do (a story), defines a word (the vocabulary), or shows and reads (a surface). Then
ask whether it is already declared somewhere, because if so it is derived and not copied.

*Paying down a ratchet.* Take the largest entry and list its nouns and verbs. Make the modules that name them. Move
whole items and never rewrite while moving, compile, run the tests, and lower the entry. Behaviour does not change
in a move, and if it must, that is a separate change.

*A module's front.*

```rust
//! What this module is for, as a noun phrase.

mod taking;                                   // one submodule per purpose, named for what it does
mod going_back;

pub use taking::take_a_snapshot;              // the verbs offered upward, and nothing else
pub use going_back::{go_back, roll_a_change_back};
```

*A worked example: `snapall delete`.* It was 55 lines that taught Rust (`.into_iter().find(|snapshot| ...)`,
`map_or(String::new(), |date| format!(...))`, `as_ref().map(..).map_err(..)`), with a bare `None` and a bare `bool`
that could not be read and a prose comment holding its only *why*. Written as a story in the language of the layer
below, it is this (the text is read from the source file, not retyped):

```rust
pub fn delete(ctx: &Context, area: &ConfigName, id: SnapshotId, destroy: bool) -> Outcome {
    if let Some(snapshot) = find_snapshot(ctx.backend, area, id)? {
        let it = snapshot_in_words(area, &snapshot);
        output::note(&what_deleting_loses(&it, area));
        ask::before(Act::DeleteSnapshots, destroy, &delete_question(&it))?;
    }
    let change = Change::begin(&deleting_one(area), &deleted_one(area), None)?;
    let deleted = delete_snapshot(ctx.backend, area, id);
    if deleted.is_ok() {
        change.rollback(after_deleting());
    }
    change.end_with(&deleted);
    deleted?;
    ctx.show.result(
        crate::render::deleted(area, id),
        crate::render::deleted_json(area, id),
    )
}
```

The wording moved beneath the story, the lookup became a verb in the core, the plumbing became `end_with`, and the
comment became two tests with those names. What was left was a three-line `if let Some(stop) = ...` repeated in eight
handlers, which was a missing noun: a story *stopped* by an answer that is not a yes. It is now `Stopped`, which
lives where it is produced, so each story says `ask::before(..)?;` and `main` ends it with its own exit code. The
syntax that remains (`?`, the trailing `None`, the bare `destroy: bool`) lists the nouns still missing: an `Approved`
in place of a flag and a question, and a name for "no name".

## What goes wrong

A prose layer that does not run is a comment: if it can be deleted and the program stays the same, it was never the
code. A front that grows has logic in it, so a level is missing. A layer that only renames the one below adds a hop
and no meaning, and should go. Names taken from a pattern (`Manager`, `Service`, `Util`) say nothing about purpose,
and a tool's word above its wrapper splits the language in two. A ratchet that never shrinks is a promise broken.
Splitting past one language, into files of ten lines, is not clarity. And cleverness in a story defeats it, since
stories are boring on purpose.

## What is not settled

The alarm limits are judgement, chosen to bite without blocking. Returning outputs as values costs a layer, and
pays where there are several surfaces or output worth testing. Whether a layer is deep enough to deserve a module is
a judgement no tool makes: the test is whether it hides something a reader should not have to know. The comments
position is contested, as said. And all of it has been tried in one project, in one language.
