<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Every module is a small language

A rule for writing software so that the code itself is the specification, and cannot drift from it. Written to
be followed by a person or by an agent: each rule says how to check it by machine.

## The idea

A program is a stack of layers. **Treat each layer as a small language**: it has *nouns* (the states it can be
in, as types), *verbs* (the actions it offers, as functions) and *a story* (how its verbs are made of the verbs
of the layer below). A reader who knows the language of one layer can read that layer's code like prose, without
knowing how the layer below is built. The stack stops where the language is not ours: the compiler, the standard
library, other people's crates, the operating system, the tools we call. There, we **wrap each foreign thing
once**, in our own words, and nothing above the wrapper ever names it.

This is stratified design (Abelson and Sussman: build abstractions by building languages), information hiding
(Parnas), deep modules (Ousterhout), ports and adapters, the stepdown rule and a ubiquitous language, held
together as one discipline instead of six habits.

## Vocabulary

* **Layer**: a level of abstraction. Each is written in the words of the one above it and calls only the one below.
* **Module**: one purpose, one language. A directory or a file, with a *front file*.
* **Front file** (`lib.rs`, `main.rs`, `mod.rs`): the *map* of the module. It says what the module is for, names
  its nouns and verbs, and nothing else. It is short enough to read in one look.
* **Surface**: where a person or another program meets the application: the command line, the window. A surface is
  a front file whose nouns are *every input a person can give* and *every output they can be shown*.
* **Boundary**: where our code ends. An *adapter* wraps what lies beyond it.
* **Ratchet**: a rule that cannot be met yet, kept honest by writing the debt down with its size: nothing may get
  worse, nothing new may join, and an entry must leave the list when paid.

## The rules

**R1. A module says what it is for, in one sentence, in its first doc line (`//!`).**
Why: it is the module's name in prose; the outline is built from it. Check: a missing line fails the build; the
outline (`OUTLINE.md`) is generated from these sentences and must equal the committed one.

**R2. A front file is a map, not contents.** It holds: the purpose sentence, `mod` declarations, the re-export of
the module's vocabulary, the declarations of its nouns, and short functions that only compose verbs. No
algorithms, no parsing, no formatting. Bound: 120 lines. If it grows past a screen you have found a missing
level, not a reason to write more here. Check: ratchet on front-file length.

**R3. Names are the person's words.** Nouns and verbs are named as a person would say them
(`take_a_snapshot`, `GoBack`, `ProtectedArea`), never by a pattern (`Manager`, `Helper`, `Util`, `Handler`) and
never by a tool's vocabulary above the adapters (`config`, `subvolume`, `snapper`, `btrfs`). A name that needs a
comment to be understood is the wrong name. Check: ratchet on tool words in identifiers above the adapters;
a glossary of forbidden words in messages.

**R4. The story calls only the layer below.** A function reads as a sentence: *ask, do, record, say* each one
call, in order. If it reaches two layers down, a level is missing; if it needs a paragraph, it is two functions.
Bound: 40 lines per function (60 while a ratchet is being paid down). Check: ratchet on function length;
dependency direction tests between crates and between layers.

**R5. Derive; do not restate; verify only what cannot be derived.** A fact is declared once and everything else is
computed from it: the command list from the contract, the flags from the weights of the acts, a button's look
from what it is for, a question's title from the act. Where two places must agree and cannot be made one (two
crates that may not depend on each other), write a test against the *single* declaration. A test that compares
two hand-written copies is a smell: remove a copy.

**R6. A surface is `input -> story -> output`, and both ends are enumerations.** Every input a person can give is
a variant of one type (`Request`, `Msg`, `Key`, `Button`); every output they can be shown is a variant of one
type or a view that is a pure function of state. Handlers *return* what is to be shown; one presenter shows it.
Nothing else prints. Then "every input and output of the app" is a list you can read, test and generate
documentation from. Check: ratchet on printing outside the presenter and the asker.

**R7. Wrap every foreign thing once, at the bottom, in our words.** One module per foreign thing (snapper, the
operating system, the terminal, a serialisation crate). It translates between its world and ours; its public
surface uses only our nouns. Nothing above names the foreign thing. Check: dependency tests per crate; R3.

**R8. Make the wrong state unrepresentable.** Where a new kind of thing must be decided about, match exhaustively
and forbid the catch-all (`clippy::wildcard_enum_match_arm`), so the compiler asks. Where an order matters
(a change must be approved before it is made), carry it in the type (an `Approved` value that only the answer to
the question can produce), so the wrong order does not compile. Check: the lint; the type.

**R9. A file is one thing.** Bound: 300 lines. Past it, find the second thing and name it. Check: ratchet.

**R10. Prose is generated.** The outline of the whole program is produced from R1's sentences; the manual's
command list, the completions and the man page from the contract. Written by no one, so it is always true.

**R11. Each layer owns a kind of logic; the top keeps only *which*, *when* and *where*.** The top layers do have
logic, in the smallest portions: choosing between named outcomes, putting things in order, arranging what sits
beside or above what, streaming inputs into stories and results into the presenter. They do not contain *how*.
Logic lives at the lowest layer that can state it without knowing the layer above: domain decisions in the
middle, translation and mechanics at the bottom. A top-layer line reads as common sense, with no knowledge of
the language, the libraries, the system or the hardware assumed, and it is also the real, running implementation:
the idea, in the flesh of working code. Check: ratchets on how often a story turns and how much plumbing it spells out.

**R12. Prose may name; only code may claim.** A module's sentence (R1) is a noun phrase that says what the module
is *about* ("Deleting a snapshot."). It never says *only*, *never* or *always*: those are guarantees, and a
guarantee in prose can be false while the build is green. Put the guarantee in a test, where it can fail. (Found
in snapall: a module whose sentence claimed to be "the one place output leaves the program" while seven others
printed by themselves.)

**R13. The *why* has three carriers, and none is a comment.** (1) *Names and types*: `Weight::Destroys` is a
reason, `Approved` is a rule. (2) *Executable facts*: a test named as a sentence about the world, including the
foreign world, that runs against the real thing (`removing_an_area_deletes_every_snapshot_and_leaves_its_files`
against real snapper), so it cannot drift. (3) *The generated outline.* Comments are kept for machine-readable
annotations only (`TODO(owner): ...`, `SAFETY: ...`, attributes). A reason that no carrier holds is a missing name
or a missing test, not a missing comment: add that.

**R14. A story has a grammar.** Each line is `noun = verb(noun, noun)` or `verb(noun)`, in this order: ask, do,
record, say. A story contains no lifetimes, generics, format strings, closures that plumb `Option` or `Result`,
bare `bool` or `None` arguments, or chains of adapters; each such construction is hidden behind a verb named for
what it *means*. What is left of Rust's own syntax in a story (`?`, `if let Some(..)`) is not to be tolerated but
read: it marks a *missing noun* (a `Stop`, an `Approved`, a `Said`) that the layer needs to name. Budgets: at
most 4 turns, 2 closures, 60 lines (40 is the aim).

**R15. Arrangement is data at the top.** What sits where (a window's tabs above its page above its status above
its buttons; the commands and the flags each takes) is a declarative tree or table of nouns, written once, and
interpreted by a lower layer that knows cells, rectangles and terminals. The top says *above*, *beside*, *fills*;
the numbers live below.

**R16. A reader learns by reading downwards.** Each word is defined before it is used, in the layer the reader is
already in or the one directly below; a module's `use` lines are its dependency vocabulary and name only the layer
below. The domain is taught by its types: each noun defined once, with its relations ("a restore point is one
snapshot in every area, taken together"). Check: dependency direction tests; a *cold read*: someone (or an agent)
given only the outline and front files should be able to say what the program does and what happens when a
snapshot is deleted. Where they cannot, a name is missing.

## Procedure

**Adding a feature.**
1. Say what it is for as one act in the app's vocabulary (a name, a title, what it weighs). If it needs a new
   word, add it to the vocabulary first.
2. Declare it where the vocabulary is consumed (the contract: `.does(Act::...)`). Flags, questions, labels follow.
3. Write its story in the module whose purpose it is, in verbs of the layer below. If a verb you need does not
   exist, write *that* in the layer below first, in that layer's words.
4. If it touches something foreign, put the translation in the adapter, never in the story.
5. Run the gates; regenerate the outline; if a ratchet complains, you made something worse.

**Where does this code go?** Ask in order: what *purpose* of the app is it (module)? what *layer* (does it name
a tool: adapter; decide what to do: story; define a word: vocabulary; show or read: surface)? is it already
declared somewhere (derive, do not copy)?

**Paying down a ratchet.** Take the largest entry. List its nouns and verbs. Make a front file that names them.
Move whole items, never rewrite while moving. Compile; run the tests; lower the entry. Behaviour does not change
in a move; if it must, that is a separate change.

## Templates

A front file:

```rust
//! What this module is for, in one sentence a person would say.

mod taking;          // one submodule per purpose, named for what it does
mod going_back;

pub use taking::take_a_snapshot;                 // the verbs, and nothing else
pub use going_back::{go_back, roll_a_change_back};

pub struct World<'a> { /* the nouns the verbs work with */ }
```

A story:

```rust
pub fn delete(world: &World, snapshot: Snapshot, answer: Answer) -> Said {
    let approved = ask_first(Act::DeleteSnapshots, answer)?;   // ask
    let gone = make(Change::Delete(snapshot), approved);       // do
    record(&gone, way_back::advice());                          // record
    say(gone)                                                   // say
}
```

## A worked example: `snapall delete`

Before, 55 lines. It *taught Rust* (`.into_iter().find(|snapshot| snapshot.id == id)`, `map_or(String::new(),
|date| format!(...))`, `result.as_ref().map(|()| String::new()).map_err(ToString::to_string)`), what `None` was in
`Change::begin(a, b, None)` and what `destroy: bool` meant could not be read from it, and the only place its *why*
was written was a prose comment.

After, 19 lines, in the language of the layer below:

```rust
pub fn delete(ctx: &Context, area: &ConfigName, id: SnapshotId, destroy: bool) -> Outcome {
    if let Some(snapshot) = find_snapshot(ctx.backend, area, id)? {
        let it = snapshot_in_words(area, &snapshot);
        output::note(&what_deleting_loses(&it, area));
        if let Some(stop) = ask::before(Act::DeleteSnapshots, destroy, &delete_question(&it))? {
            return Ok(stop);
        }
    }
    let change = Change::begin(&deleting_one(area), &deleted_one(area), None)?;
    let deleted = delete_snapshot(ctx.backend, area, id);
    if deleted.is_ok() {
        change.rollback(after_deleting());
    }
    change.end_with(&deleted);
    deleted?;
    ctx.show.result(render::deleted(area, id), render::deleted_json(area, id))
}
```

The wording moved beneath the story, the lookup became a verb in the core, the plumbing became `end_with`, and the
prose comment is gone: *a snapshot that is not there is not asked about, and the failed attempt is recorded* is
now two tests with those names (`dev/cli-consequences.py`, the audit step of CI). What remains of Rust's syntax
(`?`, `if let Some(stop)`, the trailing `None`, the bare `destroy: bool`) is the list of nouns the layer still
lacks: a `Stop` that is not an error, an `Approved` in place of a flag and a question, a name for "no name".
That is the next layer, found by reading what the story still has to spell out.

## What goes wrong

* **A prose layer that does not run.** If it can be deleted and the program is unchanged, it is a comment.
* **A front file that grows.** It is a map; if it has logic, a level is missing.
* **Pass-through layers.** A layer that only renames the one below adds a hop and no meaning. Remove it.
* **Names by pattern** (`Manager`, `Service`, `Util`): they say nothing about purpose.
* **Vocabulary leaks.** A tool's word above its adapter splits the language in two.
* **Ratchets that never shrink.** A debt list is a promise; review it when touching its neighbours.
* **Over-splitting.** A ten-line file per function is not clarity. A file is one *thing*.
* **Cleverness in a story.** Stories are boring on purpose.

## Enforcement in snapall

| Rule | Where it is kept |
|------|------------------|
| R1, R10 | `OUTLINE.md`, generated; `architecture.rs` |
| R2 | ratchet: front files over 120 lines |
| R3 | ratchet: tool words in identifiers; tests on messages |
| R4 | ratchet: handlers over 60 lines; crate dependency tests |
| R5 | `commands!` macro derives flag lists; `Act` derives titles and looks; text tests |
| R6 | ratchet: printing outside the presenter |
| R7 | `drivers_and_adapters_never_know_each_other` and the per-crate dependency tests |
| R8 | `#[deny(clippy::wildcard_enum_match_arm)]` on the change-describing functions |
| R9 | ratchet: files over 300 lines |
| R11 | ratchets: a story turns at most 4 times and spells out at most 2 closures |
| R12 | purposes are noun phrases; guarantees are the ratchets and tests themselves |
| R13 | named executable facts: `dev/cli-consequences.py`, the real-snapper tests, CI's audit step |

## What is not settled

The bounds (40, 60, 120, 300) are judgement, chosen to be strict enough to bite and loose enough to meet. The
presenter rule (R6) costs a layer of values between a handler and the screen; it pays where there are two
surfaces or output worth testing, and may not elsewhere. Whether a layer is "deep" enough to deserve its own
module is a judgement no tool makes; the test is whether it hides something a reader should not have to know.
