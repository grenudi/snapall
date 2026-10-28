# Development log

What was done to this code, and why, newest first. The plan is `docs/backlog.md`; how an iteration goes is in `AGENTS.md`. An entry says what changed, why it was chosen, and the evidence, so that a session that remembers nothing can carry on.

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
