<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# 17. Background jobs from one catalogue, room counting, and one change at a time

Status: accepted. Every part is built; the rest is designed here and staged.

## Context

After the Health and Activity tabs (ADR 16), twelve requests followed. They are answered one by one below,
because several interlock, and the right answer to a few is different from the one asked for. The
standard for each: would it be the best thing for the app, not only possible?

## Decisions

### 1. Counting the room snapshots take (quotas): built, offered, and left off by default

What it gives: the room each snapshot alone holds (otherwise unknowable), and cleanup that can act on
room as well as on numbers. What it costs: the disk keeps a tally; full counting is known to slow some
operations down, deleting snapshots most of all (the kernel's own notes on simple quotas call it a known
CPU and IO bottleneck), and the tally must be built once. It applies to the whole disk, not to one area.

Only the **full** kind is offered. The newer *simple* kind (kernel 6.7 and later) is cheap, but it charges
room to whichever subvolume wrote it first, so it cannot say what a *snapshot* holds, which is the one
question the figure answers. snapall explains this, does not offer to switch to it, and does not claim it
can convert one kind to the other (it cannot, in place).

The recommendation, stated in the text a person reads: with few snapshots and no need to see their size,
leave it off. It is never switched on silently; the window and `snapall quotas on` explain what it gives
and costs and ask first. Found by trying it on real Btrfs: enabling already starts the tally, so an
explicit rescan after it is refused as "in progress"; enabling twice is harmless.

### 2. Background jobs: one catalogue, details and controls from it

`snapall_core::keeping::jobs` names each job once (unit, plain title, purpose). The Health tab lists every job
that exists on the computer, with its **real state** and the **schedule and description its own unit
file declares** (read with systemd's override rules). Enter opens its controls: a key for each of enable,
disable, start, stop, restart and, for a timer, "run it now", each beside the command that does the same,
and the two commands that check on it (status, and the log of the unit that does the work). The command
a person is shown and the command the shell runs are made from the same words, and a test says so.

### 3. Cleanup described from real settings; editing them

Built: the description of automatic cleanup is made from the schedule in the unit file and each area's
real limits and minimum age (it used to say "50" and "once a day" in text). A test changes the inputs and
checks that the text changes with them, and that when nothing is known nothing is invented.

Built: editing them, in the Health tab (Enter on an area) and with `snapall limits`. Three settings (how many to keep, the age under which nothing goes, and whether cleanup runs), changed through the same port method that already sets snapper's options. Lowering how many are kept is the only change in snapall that destroys something later, by someone else's hand (snapper's daily cleanup), so it is **previewed**: a pure function says how many snapshots the next cleanup would remove, from the snapshots that cleanup looks after, the oldest beyond the limit, sparing those younger than the age. It was checked against snapper's own cleanup on real Btrfs, in both cases. A change that would remove something asks twice; one that removes nothing is applied at once; keeping none is refused (switch cleanup off instead). Only the limit for the snapshots snapall takes (`NUMBER_LIMIT`) is edited, not snapper's separate limit for ones marked important.

### 4. Groups of areas: built

A saved name for a set of areas, with `all` as the default meaning every protected area. Kept in a plain file, `/etc/snapall/groups`, one group a line (`theming: home, data`): readable, editable by hand, and not a format to migrate. A group names *areas*, never paths, so snapper stays the one place that says what is protected, and a group that names an area that no longer is **is refused with the area's name**, not shortened (a snapshot that quietly leaves out something the person thought was included is the worst way to fail). A file that cannot be read is an error naming the line, and the window will not overwrite it.

Used by `snapall create --group NAME`, by `snapall groups [add|remove]`, and in the window by `g` (make, change, remove) and by Tab in the snapshot prompt (all, then each group). A group is a way of choosing areas, nothing more: the snapshot it takes is the ordinary one, one restore point, and restoring it is the ordinary restore. `all` is reserved; names are plain lower-case words. Not done: tagging a restore point with the group it came from, which would need a second place to say what a restore point is.

### 5. Protecting any volume, and stopping: built

Protecting any Btrfs part of the disk already worked (`snapall setup AREA`, and the offer in the window). Stopping is the one operation here that cannot be undone, and it was checked on real snapper first: removing a config deletes the `.snapshots` folder and **every snapshot of the area**, and leaves the area's own files alone. So the warning can be exact, and is: a **plan**, shown before anything is done and made from the same words for the window and the command line (how many snapshots go, how much room that frees when it is measured, how many restore points stop including the area, which groups name it, and that it cannot be undone).

Two things make it hard to do by mistake. The area's **name must be typed** (in the window, and on a terminal; `--yes` is for scripts): a `y` or Enter, enough elsewhere, is not. And the **system area is refused**, at planning and again at doing, so no caller can reach the deletion without passing the check: its snapshots are what the boot menu offers and how you get back from a bad update (ADR 12 already never restores over the running system for the same reason). The plan is computed again at the moment of doing, not trusted from when the prompt opened. `U` is a key of its own, never the Enter that a row means for something milder.

### 6. Activity beyond this window: built. And 10–12, jobs that outlive the window: staged

**Built (6): the journal.** Every change writes an event when it takes the change lock and another when it ends, whichever snapall makes it (the window, or a command), to one append-only file of JSON lines under `/var/lib/snapall`. Folding the two gives running, done, failed or **stopped**: a change that started and whose process is gone without having said how it ended (the program was killed, the computer switched off) is shown as stopped, with what to do next, instead of vanishing or looking as if it were still running. The Activity tab and `snapall activity` read it, so history reaches back across sessions and shows what commands did. A line is written in one call so two programs cannot spoil each other's lines; the file is rotated when it grows; a line that cannot be read is skipped, and failing to remember never stops a change. Checked on real Btrfs, with a command killed by `kill -9` mid-change.

The journal is also the foundation of 10–12 below, which are one design question, and I would not answer it with a resident daemon.

A resident privileged daemon (a socket, a protocol, a queue) would satisfy the request, but it is a new
always-running attack surface, a second code path next to the one used today, and a thing to keep alive
across upgrades, for a tool whose work is a few seconds of snapper calls now and then.

**Built (10–12): the runner is the command line itself, run detached.** Building it showed a simpler shape than transient units with a job file of their own: `snapall create` and `snapall restore-all` already take the change lock, write the journal and, given `--yes`, ask nothing. So the window hands a slow, multi-area change to *the same command a person would type*, started with `--job NAME` so that the window can find its journal entry. One code path; nothing new to serialize; anything the window can start, a terminal can start, watch (`snapall activity`) and stop (`snapall cancel NAME`). Where systemd is running it is started through `systemd-run` (so the system keeps its log under `snapall-job-NAME` and the change survives the end of a login session); elsewhere as a process group of its own. One-area changes take a moment and stay in the window. Nothing runs when nothing is being done, and there is no resident daemon.

The window follows what it started through the journal, once a second while anything runs (and keeps its clock ticking for that: a window that ticked only while busy never noticed a change it had handed away), says when it ends, and then looks at its lists again. A change that never appears in the journal is let go of after thirty looks, with a word about it. The status bar shows "N running", a click on which goes to the running change in Activity, which is also the jobs view: one list of changes, whatever their state, rather than a second list to disagree with the first.

**Stopping is cooperative and never inside a step.** A multi-step change follows a *course*: before each area it is asked "may I begin step N of M?", which is also how progress is reported. Stopping is a request (a small file under `/run/snapall/stop`, named for the change) that is looked for between areas; nothing is signalled, so a snapper call is never interrupted. The steps already done stay done, the rest never start, and the change ends *cancelled* with how far it got (for a restore point, also the restore point that undoes what was done). A change killed outright (the computer switched off) is still shown as stopped, by the journal. The window asks before stopping, and says what stopping means.

If a resident daemon is still wanted, the journal, the job names and the course are what it would be built on.

### 7. Restores and the state of programs

A real concern, and the answer is in two parts. Restoring an area puts back everything in it, including
what programs keep there (settings, saved sessions, databases, logs), and a program that is still open
can overwrite the restored files from memory. Built: every restore prompt, and the command line's
confirmation, say so and say what to do (close those programs; restore one file or folder if that is all
that is needed). The structural remedy is Btrfs's own: **a snapshot stops at a subvolume boundary**, so
data that should not roll back (caches, virtual machines, containers, logs) belongs in subvolumes of its
own. The README says so.

### 8. "Working" says what, and for how long

Built: the header shows elapsed time; the line under the status bar says what is being done, in the
present tense, from the same wording as the Activity record; after five seconds it leads with "this can
take a while; waiting is safe", and when closing was asked it says so. The reassurance comes first because
a long line is clipped at the end (a test at 80 columns found that).

### 9. Two snapalls at once

Built: a lock under `/run/snapall` (emptied at every start) held for the duration of each change, by the
window and by every command that changes something. A second change is refused with who holds it and
what for; reading is never blocked. A lock whose owner is gone is taken over, and a process that is merely
starting a program is not robbed of it (the kernel reports an empty command line for a moment, which a
flaky test exposed). It guards snapall from snapall, not the disk from other tools: snapper and other
programs do not know of it.

## Consequences

The window's Health tab became the place where the state of the system is read and its jobs controlled,
and every number or name in it has one source. The remaining work is ordered: the job journal and
runner (6, 10–12), then groups (4), editing limits (3), stopping protection (5).
