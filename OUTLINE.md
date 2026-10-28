<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Outline

What each part of snapall says it is for, read from the first line of each module. Generated: do not edit. After changing what a module is for, run
`UPDATE_OUTLINE=1 cargo test -p snapall --test architecture the_outline`.

## snapall-core

- `boot.rs`: Can a snapshot be booted? The question behind "restore the system".
- `calendar.rs`: Dates, without a clock: turning seconds into a name for a restore point, and snapper's own date text into seconds so two snapshots can be compared.
- `consequence.rs`: What an action weighs, and so what has to be typed before it happens.
- `course.rs`: A change that takes several steps (a snapshot of every area, a restore point put back area by area) follows a [`Course`]: before each step it asks whether to begin it.
- `coverage.rs`: The parts of a disk that snapshots can protect, and which of them are.
- `doctor.rs`: `snapall doctor`, as pure functions: from facts about the machine to findings and an install plan.
- `domain.rs`: Validated domain types.
- `explain.rs`: What snapall tells a person about what it does, in words that assume nothing: no snapshots, no filesystems, no snapper.
- `find.rs`: Finding what a person points at: one snapshot of an area, by its number.
- `groups.rs`: Groups: a name for a set of protected areas, so that "take a snapshot of my theming" is one thing to say.
- `health.rs`: Health: is protection working, how much room does it take, will old snapshots be cleaned up.
- `journal.rs`: What snapall did, remembered: every change writes an event when it starts and another when it ends, whichever snapall made it (the window or a command), and folding them gives the record a person reads.
- `lib.rs`: Domain, planning and ports for snapall.
- `limits.rs`: How many snapshots an area keeps, and when the oldest go: the settings of automatic cleanup, and what changing them would do.
- `observe.rs`: The observability port: what snapall reports about what it did.
- `plan.rs`: Functional core: decide *which* configs to snapshot.
  - `points/finding.rs`: Finding restore points among snapshots: by their tag, or by matching what one action took together.
  - `points/mod.rs`: Restore points: the snapshots that one action took together, one in every area, found again and put back with a single action.
  - `points/putting_back.rs`: Putting a restore point back: what it would change in each area, and doing it.
- `port.rs`: The outbound port: everything snapall needs from "something that takes snapshots".
- `rollback.rs`: The way back from each kind of change, in one place: what it names as the earlier state, the commands that return to it, and what doing that weighs.
- `services.rs`: The background jobs snapall depends on or watches: one catalogue, so that a name is written once.
- `system.rs`: What the machine can tell snapall besides what snapper says: how full a disk is, whether a background job is switched on and running, what time it is there.
- `testing.rs`: Test support: an in-memory adapter and the contract every backend must satisfy.
- `unprotect.rs`: Stopping the protection of an area: the one thing snapall does that cannot be taken back.
- `usecase.rs`: The application use case, written against the port only.

## snapall-observe

- `clock.rs`: A UTC timestamp, without a date library: a log line needs only this much.
- `lib.rs`: Adapter for the observability port.
- `lines.rs`: One line per event, as text for people or JSON for machines.

## snapall-snapper

- `backend.rs`: Our snapshot verbs, said to snapper one command at a time.
- `files.rs`: Looking at a file as snapper's snapshots keep it, and comparing it with now.
- `lib.rs`: The snapshot backend for snapper: our verbs said to it, and what it says read back in our words.
- `reading.rs`: Reading what snapper prints back into our own words.
- `running.rs`: Running the snapper program and what it leaves behind.

## snapall-system

- `groups.rs`: The groups file: plain text under `/etc/snapall`, read and written whole.
- `journal.rs`: The journal on disk: one JSON object per line, appended.
- `lib.rs`: The system port, answered by asking the machine: `df` for a disk, `systemctl` for a background job, `date` for the time, unit files for a job's schedule, `btrfs` for how room is counted, and a lock file for who is making a change.
- `lock.rs`: The right to make a change, held in a file: only one snapall changes things at a time, so that two restores, or a restore and a deletion, cannot race each other.
- `quotas.rs`: How a disk counts the room snapshots take, and switching it on and off.
- `runner.rs`: Starting a change that outlives the program that asked for it, and asking it to stop.
- `units.rs`: What a job's unit file says about it.

## snapall-tui

- `changes.rs`: What a change is called in the Activity tab, and how it went: the words, apart from the doing.
- `lib.rs`: A terminal UI for snapall: a second *driving adapter* over the same port as the CLI.
  - `model/changed.rs`: What a finished change leaves behind in the window: its result, what to show of it, and what to look at again.
  - `model/file.rs`: The file history screen: one file's versions, newest first, and putting one back.
  - `model/following.rs`: Following the changes that run on their own: looking at the journal once a second while anything runs, and noticing when one that this window started has ended.
  - `model/input.rs`: What a person does: a key, a click, a button.
  - `model/keys.rs`: What each key and click does, and how a prompt answers one.
  - `model/lists.rs`: The lists of areas and snapshots: what happens when they arrive.
  - `model/living.rs`: The window's own life: starting, the clock, the pointer and a refusal.
  - `model/looked.rs`: What looking brought back arriving in the window: the areas, snapshots, history, health, groups and points.
  - `model/messages.rs`: What the window asks the shell to do (effects) and what the shell reports back (messages).
  - `model/mod.rs`: The pure core of the UI: state, messages, effects and `update`.
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
  - `model/state.rs`: What the window knows: the lists, the cursor, the open prompt, the message, and what the shell told it about the machine.
- `session.rs`: The imperative shell around the pure model.
- `status_bar.rs`: The status bar: one line under every view with the numbers that matter (how many snapshots, how much room they take, how full the disk is, whether cleanup runs, how many things need a look).
- `tab.rs`: The views of the window.
  - `tabs/activity.rs`: The Activity tab: what snapall did, newest first, and how each went.
    - `tabs/health/drawing.rs`: Drawing the Health tab: the rows, the verdict above them and the detail of the chosen one.
    - `tabs/health/keys.rs`: What pressing a key does on the Health tab: moving along the rows and acting on the chosen one.
    - `tabs/health/mod.rs`: The Health tab: is protection working, how much room does it take, will old snapshots be cleaned up, and what to do about anything that is not.
    - `tabs/health/rows.rs`: What the Health tab lists: each row's name, how it stands, what it means and what Enter does.
  - `tabs/mod.rs`: One module per tab that has more to it than the snapshot lists: each owns its state, its keys and its drawing, so a tab is read in one place.
- `term.rs`: The terminal: setup and restore, key translation, and the event loop.
- `theme.rs`: The colours, in one place, so every screen speaks in the same ones: good is green, something to know is cyan, something to do is yellow, something to do now is red.
  - `view/file.rs`: The file history screen: one file's versions, newest first.
  - `view/footer.rs`: The two lines under the status bar: the explanation of whatever the mouse is over, and the keys as buttons.
  - `view/header.rs`: The tab bar: the brand badge, the three tabs, and where a click on each lands.
  - `view/hit.rs`: Where a click lands: the same layout the drawing used, so a click can only land on what was drawn.
  - `view/lists.rs`: The Snapshots tab: the protected areas on the left, the chosen area's snapshots on the right.
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
- `boot_probe.rs`: The imperative half of the boot-menu check: look at files and services, hand the facts to `snapall_core::boot::assess`.
- `cli.rs`: Driving adapter: turn an argv and an environment into a validated [`Parsed`] request.
  - `commands/change.rs`: A change being made: it is started and written down, followed while it runs (can it be asked to stop?), and ended with how it went.
  - `commands/deleting.rs`: Deleting a snapshot.
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
- `contract.rs`: snapall's invocation surface, declared once with argenv.
- `doctor.rs`: The imperative shell for `snapall doctor`: probe the machine, run the install steps.
- `generate.rs`: The shell completions and the man page.
- `help.rs`: `--help`, generated from the contract so it can never disagree with what is accepted.
- `logging.rs`: Where the audit trail goes: nowhere, to stderr, or to a private file.
- `main.rs`: Composition root and imperative shell: wires the argenv-driven CLI, the TUI and the snapper adapter to the core, performs the I/O, and maps outcomes to exit codes.
- `output.rs`: Printing, and the choice between text for a person and JSON for a program.
  - `render/activity.rs`: What snapall has done, as the journal remembers it.
  - `render/changing.rs`: What a restore would change, what a restore or a deletion did.
  - `render/groups.rs`: Groups of areas.
  - `render/health.rs`: How protection stands, and the checks that find out.
  - `render/listing.rs`: Listing snapshots and the history of a file.
  - `render/mod.rs`: Presentation: pure functions from domain values to text or JSON.
  - `render/points.rs`: Restore points: listing them, what putting one back would do, and what it did.
  - `render/taking.rs`: What taking a snapshot reports: what was taken, or what would be.
- `setup.rs`: `snapall setup`: find the parts of the disk that snapshots do not cover, explain what that means in plain words, ask, and set them up.
