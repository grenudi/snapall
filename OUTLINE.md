<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Outline

What each part of snapall says it is for, read from the first line of each module. Generated: do not edit. After changing what a module is for, run
`UPDATE_OUTLINE=1 cargo test -p snapall --test architecture the_outline`.

## snapall-core

  - `going_back/boot.rs`: Can a snapshot be booted? The question behind "restore the system".
  - `going_back/mod.rs`: Going back: restoring an area or a restore point, the way back from each change, and whether a snapshot can be booted.
    - `going_back/points/finding.rs`: Finding restore points among snapshots: by their tag, or by matching what one action took together.
    - `going_back/points/mod.rs`: Restore points: the snapshots that one action took together, one in every area, found again and put back with a single action.
    - `going_back/points/putting_back.rs`: Putting a restore point back: what it would change in each area, and doing it.
  - `going_back/restoring.rs`: Putting an area, or one path of it, back to how a snapshot had it, with a safety snapshot first.
  - `going_back/rollback.rs`: The way back from each kind of change, in one place: what it names as the earlier state, the commands that return to it, and what doing that weighs.
  - `health/chips.rs`: The chips that say how things stand at a glance.
  - `health/facts.rs`: Collecting what is true of the machine: each area's snapshots and limits, the jobs, the room taken.
  - `health/findings.rs`: Finding what is wrong, one check at a time, and the verdict they add up to.
  - `health/mod.rs`: Health: is protection working, how much room does it take, will old snapshots be cleaned up.
  - `health/sizes.rs`: Sizes in the words a person reads.
  - `health/verdicts.rs`: What judging produces: levels, topics, remedies, findings, chips and the verdict.
  - `keeping/groups.rs`: Groups: a name for a set of protected areas, so that "take a snapshot of my theming" is one thing to say.
  - `keeping/jobs.rs`: The background jobs snapall depends on or watches: one catalogue, so that a name is written once.
  - `keeping/limits.rs`: How many snapshots an area keeps, and when the oldest go: the settings of automatic cleanup, and what changing them would do.
  - `keeping/mod.rs`: What is kept: how many snapshots an area holds, which areas go together, and the jobs that keep things tidy.
- `lib.rs`: What snapall does, with no terminal, no file and no tool in sight: protecting parts of the disk, taking snapshots, looking at them, going back to them and keeping them.
  - `looking/find.rs`: Finding what a person points at: one snapshot of an area, by its number.
  - `looking/history.rs`: The history of one file: which area holds it, and what it was in each snapshot.
  - `looking/journal.rs`: What snapall did, remembered: every change writes an event when it starts and another when it ends, whichever snapall made it (the window or a command), and folding them gives the record a person reads.
  - `looking/mod.rs`: Looking at what there is: an area's snapshots, what a path points at, a file's history, and what snapall did.
  - `looking/snapshots.rs`: The snapshots of an area: listing them, and deleting one.
  - `ports/course.rs`: A change that takes several steps (a snapshot of every area, a restore point put back area by area) follows a [`Course`]: before each step it asks whether to begin it.
  - `ports/machine.rs`: What the machine can tell snapall besides what snapper says: how full a disk is, whether a background job is switched on and running, what time it is there.
  - `ports/mod.rs`: What the core needs from outside, in its own words: a backend for snapshots, the machine, an observer, a way to be asked to stop, and stand-ins for them.
  - `ports/observing.rs`: What snapall reports about what it did: the events, and whoever receives them.
  - `ports/snapshots.rs`: What snapall needs from whatever takes snapshots, said in snapall's own words.
    - `ports/testing/backend.rs`: The in-memory stand-in doing what a real backend does.
    - `ports/testing/building.rs`: Setting up the in-memory stand-in the way a test needs it.
    - `ports/testing/contract.rs`: The contract every backend must satisfy, as one function to run against it.
    - `ports/testing/mod.rs`: Test support: an in-memory adapter and the contract every backend must satisfy.
    - `ports/testing/state.rs`: What the in-memory stand-in remembers: each area, its snapshots and files.
    - `ports/testing/system.rs`: A machine whose answers a test sets: its disk, its jobs, its clock, who else is making a change.
  - `protecting/coverage.rs`: The parts of a disk that snapshots can protect, and which of them are.
    - `protecting/doctor/diagnosing.rs`: Diagnosing the setup: what each check finds and what to do about it.
    - `protecting/doctor/installing.rs`: Understanding the operating system, and how to install the snapshot tool on it.
    - `protecting/doctor/mod.rs`: `snapall doctor`, as pure functions: from facts about the machine to findings and an install plan.
  - `protecting/mod.rs`: Protecting parts of the disk and stopping: which parts can be protected, what the doctor finds, how an area is made and unmade.
  - `protecting/protect.rs`: Protecting a part of the disk: making it an area.
  - `protecting/unprotect.rs`: Stopping the protection of an area: every snapshot of it is deleted for good (its own files are not touched), so it is *planned* first: what would go, how much room that frees, which restore points and groups it affects.
  - `taking/mod.rs`: Taking snapshots: which areas, and doing it area by area.
  - `taking/plan.rs`: Deciding which areas to snapshot.
  - `taking/run.rs`: Taking snapshots: planning which areas, doing it area by area, and reporting how each went.
  - `vocabulary/calendar.rs`: Dates, without a clock: turning seconds into a name for a restore point, and snapper's own date text into seconds so two snapshots can be compared.
  - `vocabulary/consequence.rs`: What an action weighs, and so what has to be typed before it happens.
    - `vocabulary/domain/files.rs`: What changed in a file between a snapshot and now, and what a restore did.
    - `vocabulary/domain/keeping.rs`: How many snapshots an area keeps, and the options that say so.
    - `vocabulary/domain/mod.rs`: Validated domain types.
    - `vocabulary/domain/names.rs`: Names and numbers that are parsed once, at the boundary: an area, a description, a snapshot number, a restore point.
    - `vocabulary/domain/snapshots.rs`: What is known about a snapshot.
  - `vocabulary/explain.rs`: What snapall tells a person about what it does, in words that assume nothing: no snapshots, no filesystems, no snapper.
  - `vocabulary/mod.rs`: The words snapall thinks and speaks in: its names and numbers, what an act weighs, dates, and what it tells a person.

## snapall-observe

- `clock.rs`: A UTC timestamp, without a date library: a log line needs only this much.
- `lib.rs`: Reporting what snapall did: every call to the backend timed and written as a line of text or JSON.
- `lines.rs`: One line per event, as text for people or JSON for machines.

## snapall-snapper

- `backend.rs`: Our snapshot verbs, said to snapper one command at a time.
- `files.rs`: Looking at a file as snapper's snapshots keep it, and comparing it with now.
- `lib.rs`: The snapshot backend for snapper: our verbs said to it, and what it says read back in our words.
- `reading.rs`: Reading what snapper prints back into our own words.
- `running.rs`: Running the snapper program and what it leaves behind.
- `version.rs`: Which version of snapper is installed, or why that could not be found out.

## snapall-system

- `asking.rs`: Running a program and reading what it says: what it printed, what the service manager says of a unit, and the numbers `df` prints.
- `boot_menu.rs`: Looking at the machine for what a boot menu needs, its files and services, and handing the facts on to be judged (`snapall_core::going_back::boot::assess`).
- `environment.rs`: What this machine is, for the doctor and for setup: its operating system, whether this is the administrator, and its mount table.
- `groups.rs`: The groups file: plain text under `/etc/snapall`, read and written whole.
- `installing.rs`: Running one of the steps that install what is missing, as the command it is.
- `journal.rs`: The journal on disk: one JSON object per line, appended.
- `lib.rs`: Asking the machine what the core wants to know: `df` for a disk, `systemctl` for a background job, `date` for the time, unit files for a job's schedule, `btrfs` for how room is counted, and a lock file for who is making a change.
- `lock.rs`: Taking turns at changing things: the right to make a change is held in a file, so that two restores, or a restore and a deletion, do not race each other.
- `machine.rs`: The machine snapall is running on, answering the core's questions: its disk, its jobs, its clock, how room is counted, and who else is making a change.
- `quotas.rs`: How a disk counts the room snapshots take, and switching it on and off.
- `runner.rs`: Starting a change that outlives the program that asked for it, and asking it to stop.
- `units.rs`: What a job's unit file says about it.

## snapall-tui

- `changes.rs`: What a change is called in the Activity tab, and how it went: the words, apart from the doing.
- `lib.rs`: The window: everything snapall does, in a terminal, with the keys and the mouse, over the same core as the command line.
  - `model/changed.rs`: What a finished change leaves behind in the window: its result, what to show of it, and what to look at again.
  - `model/file.rs`: The file history screen: one file's versions, newest first, and putting one back.
  - `model/following.rs`: Following the changes that run on their own: looking at the journal once a second while anything runs, and noticing when one that this window started has ended.
  - `model/input.rs`: What a person does: a key, a click, a button.
    - `model/keys/answering.rs`: How a prompt answers a key: what it returns, and which prompt a key goes to.
    - `model/keys/clicking.rs`: What a click does: a button, a row, or the prompt over them.
    - `model/keys/cursor.rs`: Moving through the lists: the cursor, jumping, scrolling, and marking snapshots.
    - `model/keys/mod.rs`: What each key and click does, and how a prompt answers one.
    - `model/keys/screen.rs`: What a key does on the screen the person is looking at: the tab's own keys first, then the keys every screen shares.
  - `model/living.rs`: The window's own life: starting, the clock, the pointer and a refusal.
  - `model/looked.rs`: What looking brought back arriving in the window: the areas, snapshots, history, health, groups and points.
  - `model/messages.rs`: What the window asks for (effects) and what comes back (messages).
  - `model/mod.rs`: What the window does when something happens: its state, what can happen to it, and what it asks for in return.
    - `model/prompts/confirm.rs`: The question before a change: what it will do, and the answer that goes with its weight.
    - `model/prompts/delete.rs`: Deleting snapshots: asking first, and what comes of it.
    - `model/prompts/describe.rs`: The prompt for a snapshot's note: what is typed, and what Enter does with it.
    - `model/prompts/groups.rs`: The groups of areas: see them, make one, change which areas one has, take one away.
    - `model/prompts/help.rs`: The help: two pages, turned with Enter, Tab or the arrows.
    - `model/prompts/job.rs`: The controls of one background job: a key for each thing that can be done to it.
    - `model/prompts/limits.rs`: The editor of how many snapshots an area keeps: three settings, each adjusted with the arrows, and the consequence of them shown as they are set.
    - `model/prompts/mod.rs`: One module per prompt: what opens it, what its keys do, and what comes of the answer.
    - `model/prompts/points.rs`: Restore points: the list of them, the preview of the chosen one, and putting one back.
    - `model/prompts/protect.rs`: Protecting a part of the disk that has no snapshots yet: one offer at a time.
    - `model/prompts/quotas.rs`: Asking before the disk starts, or stops, counting the room snapshots take.
    - `model/prompts/restore.rs`: Restoring an area from a snapshot: showing what would change, asking, and what comes of it.
    - `model/prompts/stop.rs`: Asking before a change that is running is asked to stop: what stopping means, so that nobody is afraid to do it, and that it is only a request.
    - `model/prompts/unprotect.rs`: Stopping the protection of an area: what it would do, in plain words, and the area's name typed out before it does it.
    - `model/state/looked_up.rs`: What looking brought back, as the window holds it: areas, a file's versions, a restore point's offer, what a restore did.
    - `model/state/mod.rs`: What the window knows: the lists, the cursor, the open prompt, the message, and what it was told about the machine.
    - `model/state/questions.rs`: The questions the window asks, and what each one holds while it is open.
    - `model/state/told.rs`: What the window is told about the machine, once, at the start: its tips, what could be protected, whether it may change things.
    - `model/state/window.rs`: The window at one moment: its lists, cursor, open question, message, and what it is waiting for.
  - `session/changing.rs`: What the window asks to be changed, done through the backend and the machine.
  - `session/executor.rs`: The window's executor: it performs each effect against the backend and the machine, and answers with the message that describes the outcome.
  - `session/looking.rs`: What the window asks to be looked at, answered from the backend and the machine.
  - `session/mod.rs`: The window run synchronously: the model plus an executor, as the tests drive it.
- `status_bar.rs`: The status bar: one line under every view with the numbers that matter (how many snapshots, how much room they take, how full the disk is, whether cleanup runs, how many things need a look).
- `tab.rs`: The tabs of the window, and which one is showing.
    - `tabs/activity/drawing.rs`: Drawing the Activity tab: a line for each change, and the detail of the chosen one.
    - `tabs/activity/entries.rs`: The journal's changes as the Activity tab holds them: each one, and which is chosen.
    - `tabs/activity/keys.rs`: What the keys do on the Activity tab: move along the changes, and ask to stop the chosen one.
    - `tabs/activity/mod.rs`: The Activity tab: what snapall did, newest first, and how each went.
    - `tabs/health/drawing.rs`: Drawing the Health tab: the rows, the verdict above them and the detail of the chosen one.
    - `tabs/health/keys.rs`: What pressing a key does on the Health tab: moving along the rows and acting on the chosen one.
    - `tabs/health/mod.rs`: The Health tab: is protection working, how much room does it take, will old snapshots be cleaned up, and what to do about anything that is not.
    - `tabs/health/rows.rs`: What the Health tab lists: each row's name, how it stands, what it means and what Enter does.
  - `tabs/mod.rs`: One folder for each tab, named as the tab bar names it: the snapshots, how protection is doing, and what snapall did.
    - `tabs/snapshots/arriving.rs`: What the Snapshots tab does when the areas and their snapshots arrive, and after a snapshot is taken.
    - `tabs/snapshots/drawing.rs`: Drawing the Snapshots tab: the protected areas and their groups on the left, the chosen area's snapshots on the right.
    - `tabs/snapshots/mod.rs`: The Snapshots tab: the protected areas on the left and the chosen area's snapshots on the right.
- `term.rs`: The terminal: setup and restore, key translation, and the event loop.
- `theme.rs`: The colours, in one place, so every screen speaks in the same ones: good is green, something to know is cyan, something to do is yellow, something to do now is red.
  - `view/file.rs`: The file history screen: one file's versions, newest first.
  - `view/footer.rs`: The two lines under the status bar: the explanation of whatever the mouse is over, and the keys as buttons.
  - `view/header.rs`: The tab bar: the brand badge, the three tabs, and where a click on each lands.
  - `view/hit.rs`: Where a click lands, worked out from the same layout the drawing used.
  - `view/look.rs`: How things that can be clicked look: at rest, under the mouse, and just clicked.
  - `view/mod.rs`: Rendering: `render(state, frame)` draws, and does nothing else.
    - `view/prompts/confirm.rs`: The question before a change, in the colour of its weight: yellow for what can be put back, red for what cannot.
    - `view/prompts/delete.rs`: The prompt that asks before deleting snapshots.
    - `view/prompts/describe.rs`: The prompt for a snapshot's note.
    - `view/prompts/frame.rs`: The frame every prompt shares: its size and place (clear of the three lines beneath), its buttons, and the drawing of any of them.
    - `view/prompts/groups.rs`: The groups prompt: the list, naming a new group, and choosing its areas, each with the keys that work on it said on the screen.
    - `view/prompts/help.rs`: The help: what snapshots are, then the keys.
    - `view/prompts/job.rs`: The controls of one background job: what it is, what the machine says about it, and the keys that control it, each beside the command a person could type instead.
    - `view/prompts/limits.rs`: The editor of how many snapshots an area keeps, with what the settings would do shown beneath them.
    - `view/prompts/mod.rs`: The prompts: their shared frame, then one module per prompt that says what it shows.
    - `view/prompts/points.rs`: The restore points screen.
    - `view/prompts/protect.rs`: The offer to protect a part of the disk.
    - `view/prompts/quotas.rs`: Asking before the disk starts, or stops, counting the room snapshots take: what it gives, what it costs, what will happen, and that it can be undone.
    - `view/prompts/restore.rs`: The restore prompt: what would change, and the offer to put the whole restore point back.
    - `view/prompts/stop.rs`: The question before a running change is asked to stop.
    - `view/prompts/unprotect.rs`: The question before an area stops being protected: the plan in plain words, in red where it is destructive, and the name to type.

## snapall

- `advice.rs`: What to tell someone whose file no snapshot can contain: why, and what to do about it, in snapall's own words and with snapall's own command.
- `ask.rs`: Asking before a change, and the end of a command that was not told to go ahead.
  - `cli/building.rs`: Building the request from what was resolved: one command to one request.
  - `cli/mod.rs`: Turning what was typed, and the environment, into a request that is known to be valid ([`Parsed`]).
  - `cli/parsing.rs`: Parsing what was typed: the command, its words, and what to suggest when it is mistyped.
  - `cli/request.rs`: What was asked for: the request, how it logs, and the error in asking.
  - `commands/about.rs`: The program speaking about itself: its help, its version and its contract.
  - `commands/asked_for.rs`: Everything that can be asked for, and the command that does it.
  - `commands/change.rs`: A change being made: it is started and written down, followed while it runs (can it be asked to stop?), and ended with how it went.
  - `commands/deleting.rs`: Deleting a snapshot for good, after saying what that loses.
    - `commands/going_back/mod.rs`: Going back: to a snapshot, to a restore point, or to before a change.
    - `commands/going_back/restoring.rs`: Putting one area, or one file of it, back to how a snapshot had it.
    - `commands/going_back/restoring_a_point.rs`: Putting every area back to how a restore point had them.
    - `commands/going_back/undoing.rs`: Rolling a change back to the state it recorded.
    - `commands/keeping/groups.rs`: Groups of areas: naming a set of them to snapshot together.
    - `commands/keeping/limits.rs`: How many snapshots an area keeps, and when its oldest go.
    - `commands/keeping/mod.rs`: What is kept: how many snapshots, whether their room is counted, and which areas go together.
    - `commands/keeping/room_counting.rs`: Counting the room that snapshots take.
  - `commands/looking.rs`: Looking, which changes nothing: the areas, their snapshots, the history of a file, the health, the record, the restore points.
  - `commands/mod.rs`: Everything the command line does, by what it is for.
  - `commands/protecting.rs`: Protecting parts of the disk, and stopping protecting them.
  - `commands/running.rs`: Changes that are running: asking one to stop.
  - `commands/taking.rs`: Taking snapshots: of one area, of every protected area, or of a group.
  - `commands/window.rs`: The window: opening it, on the whole or on one file.
  - `contract/commands.rs`: The commands: what can be asked for, each with the acts it does and the flags their weight gives it.
  - `contract/inputs.rs`: The inputs: every flag, positional and environment variable, and the commands each applies to.
  - `contract/mod.rs`: snapall's invocation surface, declared once with argenv.
- `doctor.rs`: `snapall doctor` at work: looking at the machine, and running the steps that install what is missing.
- `generate.rs`: The shell completions and the man page.
- `help.rs`: `--help`, generated from the contract.
- `logging.rs`: Where the audit trail goes: nowhere, to stderr, or to a private file.
- `main.rs`: The program: read what was asked, set up what it works with, carry it out, and end with the right exit code.
- `output.rs`: Printing, and the choice between text for a person and JSON for a program.
  - `render/activity.rs`: What snapall has done, as the journal remembers it.
  - `render/changing.rs`: What a restore would change, what a restore or a deletion did.
  - `render/groups.rs`: Groups of areas.
  - `render/health.rs`: How protection stands, and the checks that find out.
  - `render/listing.rs`: Listing snapshots and the history of a file.
  - `render/mod.rs`: What the command line says: each result as text for a person or as JSON for a program.
  - `render/points.rs`: Restore points: listing them, what putting one back would do, and what it did.
  - `render/taking.rs`: What taking a snapshot reports: what was taken, or what would be.
  - `setup/flow.rs`: The setup conversation: find what is not protected, explain what that means, ask, and set it up.
  - `setup/inputs.rs`: What the setup flow is given: the facts about the machine, what was asked for, and how it talks and acts.
  - `setup/mod.rs`: `snapall setup`: find the parts of the disk that snapshots do not cover, explain what that means in plain words, ask, and set them up.
  - `setup/offers.rs`: What the window is told it could protect, and the sentence that says so.
  - `setup/report.rs`: What setup did, as a report: each area and what happened to it.
  - `setup/this_machine.rs`: Setup on this machine: its facts read from it, and a terminal or a silence to talk through.
