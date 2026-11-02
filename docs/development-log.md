# Development log

What was done to this code, and why, newest first. The plan is `docs/backlog.md`; how an iteration goes is in `AGENTS.md`. An entry says what changed, why it was chosen, and the evidence, so that a session that remembers nothing can carry on.

## 2026-11-04: a third read, and the plural that was written twenty-four times

**Why.** A third seeded read (seven modules; one was a repeat). Real findings again, in a different mix.

**What.**
- `looking/history.rs` wrote the rule "the path is under the area's location, yet not covered by it means it is elsewhere" twice, as two accumulators in `area_covering` and again in `file_versions_of`. It is one answer, `Holding`, that both use. This is behaviour-bearing logic, so I proved the riskiest line: making the shortest mount point win instead of the longest fails `the_area_with_the_longest_mount_point_covers_a_file`, and only that. (The tie rule, the first of two equally long ones wins, is kept with `min_by_key` on `Reverse`, because `max_by_key` would silently pick the last.)
- `commands/window.rs` held a path resolver and a clock that have nothing to do with the window. They are `absolute_path.rs` and `moment.rs`, and the phrase two commands each wrote (`fresh_point(backend, unix_now())`) is one verb, `a_restore_point_for_now`. A helper in `taking.rs` called `areas_in_words` actually joined area names and shared its name with a different helper in the window; it is `names_of_the_areas`.
- `RowAction::CountRoom(bool)` was the on/off of room counting yet again. My search two days ago looked for named fields (`turn_on`, `on: bool`) and missed the tuple variant. It is `CountRoom(Counting)`, which also made two match arms one.
- `how_it_went`'s `Created` arm built a list, converted it to a list of references and passed it to a closure no other arm used any more: twenty lines whose output was "`area: why` for each failure".

**The plural.** Counting the idiom `if n == 1 { "" } else { "s" }` found **twenty-four hand-written spellings** across the core, the command line and the window, and two helpers called `areas_in_words`. One core function now says it (`counted(3, "snapshot")`, with a test), and a guard counts the hand-spelled idiom per file with the usual ratchet: it may only shrink, and a new file using it fails (shown to bite by adding one to a clean file). Fifteen were converted by reading each sentence, none rewritten; nine remain in five files, for shapes `counted` does not fit. The outcomes use the same words as before, so the golden output is unchanged.

**What I would do differently.** Search for a smell by its *type shape*, not only the names it had in the place I noticed it: `(bool)` in a tuple variant is the same fact as `turn_on: bool` in a field.

## 2026-11-03: the second batch from the read

**Why.** The same seeded read that found the bare on/off found four smaller things, each in a different layer.

**What.**
- **`restore` could not use the way-back helper** from the day before, because its way back needs a value from the result (the safety snapshot's number), so it still wrote the ritual by hand. The helper's shape was too narrow: its closure now receives what the change returned, and `restore` is one line like the others. The helper is used at six sites; the rule (a way back is written down only when the change went through) lives in one place.
- **`on_click` was a table whose arms were small programs**, one of which repeated a rule written elsewhere (showing Activity reads the history again, in `on_tab_key` and here). It is a table of named verbs now, in the same order, with `switch_to_tab` shared. It was one of the long functions (82 lines).
- **The rollback producers still said `config`** in parameter and closure names, which the tool-word guard cannot see because it checks declared names, not parameters. And the snapper backend's trait methods named their `AreaName` parameters `config`; they are `area`, and snapper's own fields and messages keep its word, since the adapter is where it belongs.
- **The backend parsed the same `list-configs` document in two places**, line for line; it is one method.

**What the guards said.** Two failed before this could ship. The canon's embedded example of `delete` was stale again (regenerated from the source). And the plumbing guard failed on `make_a_group`, which is worth recording because the cause was subtle: it counts closures *with* parameters and does not count a zero-argument `||`, so changing the way-back closure from `||` to `|_|` exposed plumbing that was already there: `made.set(..).map_err(|e| e.to_string()).and_then(|()| system.save_groups(&made))`. That chain is a verb waiting for a name (set it, then save it, and word either refusal the same way), so it is `set_and_save`, rather than the budget being raised or the closure hidden.

## 2026-11-03: room counting is switched on or off, in every layer

**Why.** A second seeded read (eight more modules) found that `after_room_counting(turned_on: bool)` in the core passed the on/off as a bare bool, the same fact I had named only in the command layer the day before, which made a second, duplicate noun. Measuring it: about 35 sites in four layers carried the direction of room counting as a bool (`turn_on` in messages, effects, modals and handlers; `on` in the machine port and the core's rule), and the window wrote `if turn_on { "on" } else { "off" }` five separate times, the word for a direction written out by hand each time.

**What.** One noun in the core, next to `QuotaMode`: `Counting::On` and `Counting::Off`, with `word()` (what is said and typed), `is_on()` (for the one boundary that takes a plain yes or no, the `btrfs` call) and `opposite()` (what switching it back is). It replaced the bool in the core's rule, the machine port, the rollback, the command (its own local copy of the noun is gone), and all of the window (messages, effects, the modal, the handlers, the drawing, the tests). The five hand-written words are `to.word()`.

**What the guards did.** The length guard failed twice, honestly: the machine port file crossed 300 lines with the new noun, and the in-memory machine used by tests (at 297) crossed it with three more lines. Both were read for their seams instead of trimmed. The machine port is now four languages (what the machine reports, the probe trait, room counting, and making a change one at a time); the stand-in is its arrangement and inspection against how it answers. The splitter placed every item and its comment check passed.

## 2026-11-02: reading eight modules at random, as a stranger

**Why.** With every guard at zero and the plan nearly empty, the proxies were exhausted: they are all things I thought to measure. The cold read the automation cannot replace is reading code with no memory of having written it. I sampled two modules per layer at random with a fixed seed (so the choice could not drift to the ones I like) and read each in full.

**What I found, and what was fine.** Five of the eight had something real; three were fine as they were (`doctor/installing/plan.rs`, `snapper/files.rs`, `system/machine.rs`: the comments carry *why*, and the tools' words are where they belong).
- `ports/testing/contract.rs`: the assertion messages named functions that no longer exist (`list_configs`, `config_subvolume`, "a missing config"), because the tool-word guard checks identifiers and not strings; and it was one 100-line function whose five phases were announced by comments. It is six named phases with current words, every assertion kept, and it passes against real snapper.
- `commands/keeping/groups.rs`: the ritual *do it; if it worked, write down the way back; end the change with the result* was copied by hand in five commands, so a command that forgot the `if` would record a rollback for something that failed. It is one method, `Change::end_with_way_back`, with the way back worked out only when the change went through.
- `model/keys/screen.rs`: quit and help were written inline here and again on the file screen (now two shared verbs), and "the keys that start work" was an anonymous `matches!` inside a condition (now `starts_work`).
- `questions/restore/asking.rs`: two sentences were copied word for word into the restore-points screen, and five places wrote "area(s)" where the core says "1 area" or "2 areas". The shared words are one small module, and two tests that asserted the old plural were updated.

**What the lesson is.** Each of these is a thing a guard could have been written for, after the fact. The cold read is how you find the next guard, not a substitute for the ones you have. `dev/sample-modules.py` makes it a routine.

## 2026-10-31: the booleans that were nouns, and two that I had added

**Why.** Re-running the trend after the outline audit showed bare boolean parameters in the commands had gone from 8 to 10: two of my own refactors had added them (`json: bool` passed into two of the `setup` helpers when the context already knows). Looking at all ten sorted them into: two mine, one real duplication (`Context.json` kept a second copy of a fact `Show` already holds), three legitimate (the flag-to-noun conversion in `Mode::from_dry_run`, and the invocation facts `hints` and `no_dbus`), and five unnamed nouns.

**What.** The `setup` helpers ask the context, and `Context.json` is gone. `stopped` moved into the core's `Report` (the restore side already keeps it inside `PointRestore`), which removed a `(Report, bool)` tuple, two parameters in the command and five test destructurings. `say_the_limits(…, true)` and `(…, false)` became `Wording::Changed` and `Wording::AsTheyAre`, the classic literal whose meaning a reader cannot see at the call site. `turn_on: bool`, four times, became `Counting::On` and `Counting::Off`, with a plain yes or no only at the three boundaries that need one (the core's rule, the machine's call, the rollback). Bare booleans in the commands: 10 to 1, and the one left is the boundary.

**What the lesson is.** A metric that goes the wrong way after a refactor is information, not noise. The two I added would have stayed unless I had measured after the change, not only before it.

## 2026-10-30: the outline's sentences, audited as a set

**Why.** With every guard at zero I needed a new way to find weak spots. The outline's sentences are what a stranger reads first, and I had checked them one question at a time but never as a set. Looking for the shortest, any used twice, and any that open with a generic noun.

**What it found.** Mostly decent sentences, and one thing I had caused: **ten modules repeated their sibling's sentence word for word.** When each question became a folder I copied the asking file's first sentence into the folder's front, so the outline showed the same line twice and said nothing about how a folder differs from the file inside it. Three more were too thin to help ("Validated domain types.", "Groups of areas.", "The restore points screen."). And the question drawing files did not begin "Drawing the …" as the tabs' do.

**What.** Distinct sentences for the ten fronts, fuller ones for the thin three, a consistent "Drawing the …" for the twelve question drawing files, and a guard that would have caught my mistake: no two modules may describe themselves in the same words. Rewording the fronts moved one ranking, and the cause was in the ranker, not the sentences: it did not know that "drawn" is a form of "draw", the same blind spot as "taken" and "delete" before; it now knows that and "chosen". 48 questions, all within the first three, 35 first (31 before).

## 2026-10-30: measured instead of worried: the script tests do not race

**Why.** The plan carried an item that three tests, which write a script and run it at once, might hit the `text file busy` race the background runner's test once had. Nothing had shown it. A fast test (0.4 seconds a run) can simply be run many times.

**What.** 250 sequential runs of the binary's tests and 490 of the snapper adapter's (250 sequential, 240 four at a time): no failure and no such error. The first attempt, four copies of the binary's suite at once, failed 206 of 400 times, and reading the message showed why: "Another snapall is making a change right now". The restore tests take the machine's real one-change-at-a-time lock, so two copies refuse each other. An invalid experiment, found by reading the failure instead of counting it; the trap is in `AGENTS.md`. The item is gone from the plan with the measurement recorded.

**Also.** The first release with the retrying package install (0.28.0) ran it in 15 jobs and none needed a retry; the plan says the hypothesis is still untested. I first counted 15 retries by matching the log, which prints the step's script text (with the same `echo` line); matching only lines with the attempt number filled in gave none. Counting evidence needs the same care as writing it.

**A slip in shipping it.** The pull request had a merge conflict and so got no CI, and `ship.py` waited on the silence. The branch had been made from the old state because a line that failed to parse under `sh` skipped its own `git pull`. The script now says when a pull request conflicts with main and force-pushes only its own rebased branch; branching from `origin/main` explicitly is a trap in `AGENTS.md`.

## 2026-10-30: the last guard debts are paid

**Why.** After the huddled files, the guards' data file still held four kinds of debt besides the contract table: the window model's front at 140 lines, the `setup` handler over the branching budget, and seven names carrying a tool's word (`config`, `snapper`, `systemctl`) above the adapters. Reading each rather than shaving it:

**What.** `model/mod.rs` held three things that were not a front: the update loop and its table of messages (now `model/updating.rs`), a constant about how long a click looks pressed (now with the pointer it describes), and a pure naming rule about jobs (`title_of`, now in the core's job catalogue with a test, used by two window files). The `setup` handler is its steps with four named helpers (the options from the flags, beginning only when it is not a rehearsal, holding the conversation to a terminal or silently, and ending with the way back), like `restore_all` and `undo`. The seven names say what the value is: `area_name`, `snapshot_tool`, `listed_areas`, `by_area` and `locations`; the core's `JobAction::systemctl()` is `control_words()`.

**A milestone.** The guards' debt file now holds one entry, the contract's table of flags, which is one language and stays. Entry files, handler length, branching and plumbing, prints outside the presenter, tool words in names and the cold read's gaps are all empty. That does not mean the code is as it should be: the guards measure what I thought to measure.

## 2026-10-30: the openSUSE flake was ours, and I had mislabelled it

**Why.** The openSUSE container job had failed three times at the step named `A user's tools`. I had written down, in `ship.py`, in `AGENTS.md` and in the plan, that this was "the runner failing in its own setup, before any of our code ran", and had the script re-run it. Checking what the step actually is: it is in our workflows, and it runs `${{ matrix.setup }}`, a package install (`zypper --non-interactive install sudo gawk curl` on openSUSE) inside the container, over the network. A failure there is a mirror or network hiccup in our own step, not the runner.

**What.** The step now retries the install up to three times with a growing pause, in both workflows that have it (the install snippets and the repositories), for every distribution: a command that works is not repeated and one that never works gives up with a failure (both checked in a plain shell). The three places that described it wrongly are corrected. `ship.py` still re-runs a job that fails at that step once, but now says why correctly. It is a hypothesis that the retry is enough, not a proven fix: if it still flakes after the next releases the next thing to try is the image from `registry.opensuse.org` instead of Docker Hub.

**Evidence.** The YAML parses, and CI runs these jobs on the pull request itself.

## 2026-10-30: the huddled files, and a defect in my own tool that had been deleting documentation

**Why.** Fourteen files sat between 250 and 299 lines, just under my size limit: the sign that the limit was partly gamed. Rather than guess, I read them. Ten are one language near its natural size. Four showed the sign: two said "and" in their own purpose sentence, one held the events and the folding of events into history, one was three kinds of explanation.

**The defect.** Splitting them with `dev/split-module.py` exposed that the tool's loop after the imports skipped every line starting with `//`, and a doc comment starts with `//`. So **the doc of the first item after the imports was silently deleted in every split**. Scanning git history for every file split into a folder found 18 splits and 14 doc lines lost (`Facts`, `Load`, `Request`, `ParseError`, `AreaFacts`, `PACKAGE`, `on_tab_key`, `outcome_of`, `KEPT`, `MATCH_WINDOW_SECONDS` and others), the oldest of them from the first splits of this arc. The doc of the other items was safe.

**What.** The tool now starts the first item at its doc comment and attributes, and refuses to write anything unless every comment of the original is in what it is about to write; it also handles a module with no imports. The lost docs were restored from their original text in git, each above the item that had lost it (one sentence still said "config" and was corrected). The four splits were then made with the fixed tool and checked: no comment lost, 139 lines accounted for. `journal`, `installing`, `coverage` and `explain` are folders; their public paths are unchanged.

**What I would do differently.** I checked that comments survived after the first report of a dropped comment, but only for plain comments. A fix for the symptom I could see was not a check for the property I cared about. The property (nothing is lost) is now enforced by the tool before it writes, and the history scan is how I found the rest.

## 2026-10-29: the core understands its own way back

**Why.** The finding from the previous change: `check_the_way_back_still_exists` parsed the recorded command words back (`["restore", area, id, ..]`) to find out what a rollback would go back to, while each producer in the core assembled those words by hand. Two sides of one language with nothing tying them together: a change to either would silently break undo, and nobody would find out until someone needed it.

**What.** The stored words stay as they are (a readable, stable form that old journals depend on), and the core now reads them: `Undo::goes_back_to()` says, typed, which snapshots and restore points a rollback names, and `going_back::still_there::still_there(backend, undo)` is the check, with the exact messages a person saw before. The binary's function and its parsing are gone, and with it the last plumbing debt in the handlers. Five tests, one of which is a guard over the producers: what each producer writes must be something this can read. Shown to bite by renaming the command word one producer records (that test, and only that one, failed; the file was restored exactly).

**Evidence.** The core's tests and the fast gate; CI runs the consequences script, which undoes real changes on real snapper.

## 2026-10-29: restore-all and undo read as the stories they are

**Why.** The stories are the layer I had judged weakest: the last measurement showed the handlers' length and branching budgets as the only debts left in the command line. `restore_all` was 79 lines and `undo` 61, each with its steps (find, preview, ask, begin, do, record, end, show) written inline, and `restore_all` with the *words* of two outcomes inside it.

**What.** The words went to the core, where they can be tested: `PointRestore::how_it_went` (how many areas were put back, in the singular or the plural, and where the way back is; or why it failed) and `PointRestore::stopped_words` (how far it got, for the person and for the record), with five tests against real restores on the in-memory backend. `restore_all` is now its steps plus three named helpers (`anything_to_change`, `put_every_area_back`, `end_the_change`); `undo` is its steps plus four (`the_change_called`, `the_way_back`, `say_what_it_would_run`, `run_the_way_back`). The exit code, the order of what is said and what is recorded are unchanged (the golden output and the consequences script in CI check it). `handlers_too_long` in the data file is now empty, and `restore_all` and `undo` have left the branching list.

**A finding, written into the plan, not tidied away.** The remaining plumbing debt, `check_the_way_back_still_exists`, parses the recorded command words back to find out what a rollback would go back to. The rollback is stringly typed, and the real fix is a typed step.

## 2026-10-29: describing a change is its own language

**Why.** After a re-measurement (the trend was flat since 0.27.0, because the window's structure work is barely visible to those proxies), the biggest remaining file in the window was `changes.rs` (420 lines), holding three of the longest functions. It was three languages: what a change is called, the command that carries a change out in the background, and how it went with the way back from it.

**What.** `changes/{describing,background,outcome}.rs` (15 to 201 lines), split with `dev/split-module.py`. Doing it found a defect in the tool: it silently dropped any plain `//` comment at the top level, and a comment directly above an item made it mistake the comment for the item's head (it stopped with "unplaced", which is why nothing was lost). It now keeps every comment with the item below it; the three "exhaustive on purpose" notes survived. The measuring script also now says what is wrong when asked about a tag that is not fetched, instead of failing with an unhelpful error.

**What the cold read said.** Asked what a change is called in the Activity tab, the outline led to the Activity tab's own files (6th), because the new sentence never said where its words are shown. It now says the Activity tab's record. The three new questions: 41 asked, 40 within the first three, 25 first.

**Not changed.** The long functions in it (`how_it_went` 114 lines, `describe_now` and `describe_change` 81) are exhaustive matches over every kind of change, on purpose (a new kind must be given its words and its result, and the compiler says so), so the file got smaller and the function count did not.

## 2026-10-29: the file history screen is one place

**Why.** The last window item in the plan: the screen shown when the window is opened on one file had its decisions in `model/file.rs` and its drawing in `view/file.rs`, while every tab and every question was already a folder.

**What.** `file_history/` with `keys.rs` (what the keys do), `arriving.rs` (what happens when the versions arrive) and `drawing.rs`, and a front that says it is read in one place. The split followed the three real boundaries in the old file. It compiled first time and all 185 window tests pass unchanged. With it, every part of the window is a folder named for what it is: the tabs, the questions, and this screen; and the role-based purity rule covers all of it.

**What the cold read said.** The new screen answers its two questions (first and second). It also moved the long-standing key gap from fifth to sixth, because its own sentence competes for "what does a key do in the window". Looking at the ranking showed the better answer was the front of the key handling, which I had not listed and whose sentence did not say *window*; it now does and is accepted, and the findability debt list is empty for the first time: 38 questions, all within the first three, 24 first.

**Evidence.** The window's tests and the fast gate; CI runs the terminal journeys, which open the file screen.

## 2026-10-29: each question is one place

**Why.** The window's thirteen questions each had the same two halves in two parallel trees: `model/prompts/<q>.rs` (what opens it, what its keys do, what comes of the answer) and `view/prompts/<q>.rs` (how it is drawn). The tabs had already solved this with a folder each.

**What.** `questions/<q>/{asking,drawing}.rs` for each question, `questions/frame.rs` for the frame they share, and two small facades in `questions/mod.rs` (`asking` for the model, `drawing` for the view) that replace the two old fronts. The files moved with `git mv`; because they were descendants of `model/` and `view/` they had relied on those modules' private imports, so each was given its own (the same lesson as the tabs).

**A stronger rule, found on the way.** The test that keeps the model pure was path-based (nothing in `model/` or `session/` may use the drawing library). Counting where the library was used showed it appeared outside `view/` only in files named `drawing.rs` (plus the status bar, the terminal and the theme), so the rule became role-based: only files that draw may use it, wherever they live. It now also covers every question's asking half and every tab's keys, state and arrival code, and it was shown to fail in both.

**What the cold read said.** The new structure answers questions about it well (the limits editor and the groups question rank first). It also found two real problems, one of them caused by this change: the unprotect question did not say "stop protecting", the phrase the vocabulary itself uses (13th), and thirteen new folder fronts pushed `ask.rs` from the top three for "what is asked before a change that cannot be undone" (9th) because its sentence never said what it asks. Both sentences were rewritten. A third was a claim that had gone false: the unprotect question called itself the one prompt where Enter alone is never enough, which stopped being true when deleting a snapshot began to need a typed word. It now says what it does and claims nothing.

**Evidence.** The window's 185 tests; CI runs the terminal journeys, which open every question.

## 2026-10-29: the window's state is composed, and the Snapshots tab owns its own

**Why.** The window's `State` was one 26-field struct. Rather than group by a guess, I counted which parts of the window use each field. Three clusters were clear: the fields for *what the window is waiting for* (`pending`, `spinner`, `doing`, `busy_ticks`, `watching`, `poll_ticks`, `after_reload`) are used only by the update loop and the spinner and by no screen; the mouse (`hover`, `pressed`); and the Snapshots tab's own five (`areas`, the two cursors, `marked`, `focus`), which was the one tab still keeping its state in the global struct while Health and Activity own theirs. `health_asked` was the Health tab's all along.

**What.** `Waiting` and `Pointer` (in `model/state/`), and `SnapshotsTab` (in `tabs/snapshots/state.rs`, with the six methods that used only its fields), so `State` has 14 fields and `window.rs` went from 179 to 104 lines. About 160 uses across the source and the tests were rewritten by a script on the receivers; the compiler found the seven it missed (multi-line method chains and an import), and all 185 window tests pass unchanged apart from those renames.

**Evidence.** The window's tests and the fast gate; CI runs the terminal journeys, which drive the real window through every screen.

## 2026-10-29: everything is said through the presenter

**Why.** The plan's item was "outputs as values: handlers return what is to be said and the surface prints it", because fifteen prints bypassed the presenter. Looking at the fifteen showed the premise was wrong: all of them were lines spoken to the person on stderr (an error, a warning, "snapall will run, as root:", "To roll back:"), and several are said right before an answer is read, so speaking and listening interleave and a story cannot return its words as one value at the end.

**What.** The decision was kept and the small change made: the fifteen raw prints now go through `output::note`, the single place that prints, which is the same stderr line byte for byte. The ratchet of prints outside the presenter went from fifteen to none and its entry in the data file to `{}`, so it is now an absolute rule. The plan records the decision and its reason.

**Evidence.** The golden output of 20 commands and the consequences script (in CI) show the output unchanged, and the architecture test allows no print outside `output.rs` and the asker.

## 2026-10-29: a change is written down in one place

**Why.** Item 2 of the plan said the command line and the window each had their own lifecycle for a change. Before building anything I checked that against the code, since the item came from an older design: it was true. The window's executor took the one-change-at-a-time lock, recorded *Started*, ran, recorded *Rollback* and *Finished* with the elapsed time (about 45 lines); the command line's `Change` did the same four events and added *Progress* and a *Failed* record for an early exit (about 170 lines).

**What.** `snapall_core::InProgress` (`crates/snapall-core/src/changing/in_progress.rs`) is the lifecycle, stated once: `begin`, `rollback`, `proceed` (progress, and whether a request to stop has been made), `end`. It does not store the machine, so a change can be held by whoever owns the machine or by someone borrowing it. The command line's `Change` is now a thin wrapper that keeps what is genuinely its own (where the change came from, what it is called, the typestate that a change begins only with leave, and a *Failed* record when a command leaves early); the window's executor calls the same four methods. Four tests state the lifecycle against the in-memory stand-in, including that a refused change leaves no record of having started.

**A defect of mine, found and repaired here.** When the measurement change rewrote the backlog's Structure section by script, it looked up the table header with a search that finds the first match, which is in the Product table above, so the section lost its heading and its introduction and the rows ended up under Product. Every path in the document still existed, so nothing failed. The section is restored, and a test now checks that the backlog keeps the four sections it is read by. The lesson, for `AGENTS.md`: a script that edits a document must assert on what it expects to find, as one that edits code does.

**Evidence.** The workspace tests and the fast gate; CI runs the fixture checks, which exercise both paths: the consequences script and the golden output for the command line, and the terminal journeys for the window (the Activity tab shows what the journal recorded). The cold-read test also moved: the new module is an acceptable answer to "where is a change started and recorded", and `contract/inputs.rs` now says it is the command line's flags, declared once.

## 2026-10-29: the answer and the mode are nouns, and the weight decides what counts

**Why.** Item 1 of the plan, from the measurement: 34 bare booleans in the commands. Looking at them showed three flags making up almost all of it (`yes` 24, `dry_run` 12, `destroy` 10), travelling together, and a design flaw behind the naming: each handler chose which flag to pass to `ask::before` (`if removes { destroy } else { yes }` in limits, the same choice in undo), which is a rule the act's weight already owns (`--yes` cannot answer a question about what is lost for good).

**What.** `Agreed` (in `ask.rs`) is what the person already agreed to on the command line, and `covers(weight)` says whether that answers a question of that weight; `ask::before`, `gate` and `before_showing` take it, so no handler chooses a flag any more. `Mode` (`Really` or `ShowOnly`) is whether to do it or only show what would be done. The command line still parses booleans, and the request table (`commands/asked_for.rs`) turns them into the two nouns at the boundary. Bare booleans in the commands: 34 to 9; the nine left are other kinds of thing (see the plan). A test pins the rule itself.

**Evidence.** The fast gate; CI runs the fixture checks, including the consequences script that exercises `--yes` and `--destroy` against real snapper, and the golden output of 20 commands.

## 2026-10-28: measure the real goal, and automate the loop

**Why.** Progress toward "every layer is the top of the layers beneath it" had been judged by file sizes and ratchets, which are proxies. A measurement over the release tags (`dev/structure-metrics.py`) showed what the reports had hidden:

| | 0.21.0 | 0.22.0 | 0.23.0 | 0.24.0 | 0.25.0 | 0.26.0 | now |
|---|---|---|---|---|---|---|---|
| source files | 100 | 103 | 103 | 125 | 172 | 178 | 196 |
| median file (lines) | 122 | 125 | 125 | 125 | 107 | 100 | 90 |
| files over 300 | 19 | 19 | 19 | 16 | 6 | 5 | 2 |
| files 250-299 (huddled under the limit) | 9 | 9 | 9 | 11 | 14 | 14 | 14 |
| biggest file | 1407 | 1304 | 1304 | 845 | 515 | 515 | 515 |
| entry files over 120 | 5 | 5 | 5 | 4 | 3 | 2 | 1 |
| core: top-level entries | 22 | 23 | 23 | 24 | 9 | 9 | 9 |
| functions over 60 lines | 32 | 33 | 33 | 29 | 28 | 29 | 29 |
| prints outside the presenter | 28 | 28 | 28 | 15 | 15 | 15 | 15 |
| bare bool params in commands | 21 | 21 | 21 | 35 | 35 | 34 | 34 |
| window State fields | 29 | 29 | 29 | 29 | 29 | 26 | 26 |
| modules with no purpose sentence | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| purposes in pattern jargon | 17 | 17 | 17 | 16 | 14 | 2 | 2 |

**What it says, honestly.** Structure and naming moved steadily: files over 300 lines fell from 19 to 2, the median file from 122 to 90 lines, the core opens as 9 purposes instead of 22 modules, purposes written in pattern jargon fell from 17 to 2. The story layer barely moved: functions over 60 lines 32 to 29 (about half are legitimate one-language tables), 34 bare booleans, 15 prints that bypass the presenter, the window's state still one 26-field struct. My own size limit was partly gamed: the files huddled just under it grew from 9 to 14. A rough, unmeasured judgment: structure and names are most of the way there, the downward face of each layer (a body written only in the words of the layers beneath it) is under half done.

**The real goal as a test.** `crates/snapall/tests/findability.rs` asks whether the generated outline alone leads a stranger's question to the right module (31 questions in `crates/snapall/tests/findability.json`, a deterministic ranking, no model). First result: 30 of 31 within the first three modules, 19 first, after fixing the ranker's English (taken and taking, delete and deleting) and three sentences that did not say what a stranger would say (a module about how heavy an action is never said heavy; one about running in the background never said background; the snapper adapter never said it talks to snapper). One gap is recorded as a debt. The test was shown to fail when a purpose sentence is emptied of meaning.

**Automation, so the loop costs less.** `dev/gate.sh` runs the whole fast gate in about a minute with one line per step. `dev/ship.py` pushes, opens the pull request, waits for CI, merges, waits for main, merges the release pull request and verifies the release; it keeps no state and can be run again. The golden-output check now runs in CI, and `.github/workflows/structure.yml` computes the trend on every push to main, so it is read from the run summary and never computed by hand. The ratchet code is shared by the architecture and findability tests.

**Next.** The plan's first four items, in order of value: name the booleans, one layer that owns a change, outputs as values, the window's state by screen.

## 2026-10-27 and 28: the machine behind adapters, tabs as folders, setup (0.26)
- PR 93, 95, 97: the boot-menu probe, the doctor's reading of the machine and setup's own `systemctl` moved behind the machine and snapper adapters; setup was found to be a good conversation and its backlog item was reversed with its reason.
- PR 91, 90, 86: the three tabs are three folders named as the tab bar names them; the window's questions are a module; a question holds the answers that belong to it, so an answer for a closed question has nowhere to land.
- PR 88: the refactoring tools and the plan moved into the repository; debts became data that can only be lowered.

## 2026-10-27: the core by purpose, the program reads as itself (0.25)
- PR 82: the core's 25 modules became folders for what snapall is for (protecting, taking, looking, going back, keeping, health, vocabulary, ports).
- PR 84: `main.rs` became the story of the program and the table of requests calls each command through the name of its purpose; 21 module purposes rewritten, one of them false.

## 2026-10-26 and 27: small languages and the vocabulary (0.23, 0.24)
- ADR 19 and 20: the code states what the app is for (acts and their weights), every module is a small language; the canon text `docs/canon/small-languages.md`; the generated `OUTLINE.md`.
- The area vocabulary replaced the tool's word `config` in 81 identifiers and about 2,000 places, with the output of 24 commands identical.
