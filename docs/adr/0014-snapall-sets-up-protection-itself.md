<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# 14. snapall sets up protection itself, and teaches what it is doing

Status: accepted

## Context

snapall used to snapshot the snapper configs that already existed and say nothing about the rest. On
Arch and CachyOS `/` and `/home` are separate Btrfs subvolumes, and a snapshot stops at a subvolume
boundary, so a machine with only a `root` config had none of its owner's files protected, and nothing
said so. When the owner asked for the history of a file in `/home`, snapall answered "did not exist"
in every snapshot, which was false: it never could have been there.

The first fix printed the `snapper` commands that set up the missing config. That is the wrong shape:
it asks a person to leave the tool, to run commands in a vocabulary (config, subvolume) that the tool
exists to hide, and to trust that they did it right.

## Decision

1. **snapall does the setup.** `snapall setup` and the window's `p` find the Btrfs areas of the
   machine, say which have snapshots, explain, ask, and do it: create what the snapshots are kept in
   and turn off snapshots on a schedule. If the second step fails the first is undone, and what could
   not be undone is named.
2. **Snapshots on a schedule are turned off for what snapall sets up.** Verified on real snapper: a new
   area starts with them on, so left alone it would start taking snapshots by itself. snapall's
   snapshots happen when asked.
3. **Vocabulary.** A person reads *protected area*, never config or subvolume. One module
   (`snapall-core::explain`) holds every explanation so the command line and the window say the same
   thing, and tests fail if config, subvolume, Btrfs, snapper or filesystem appear in anything a
   person can reach: the explanations, `--help`, every screen at 80x24, the errors.
4. **Every button says what it does** on the footer line while the mouse is over it, in the context it
   is in (Enter means different things in different prompts). Keyboard users get the same sentences in
   the two-page help, which begins with what a snapshot is.
5. **Claims are checked against the machine, not assumed.** Automatic cleanup is reported *on* only if
   its timer is enabled **and** running; otherwise snapall says it could not find out.

## Verified on real Btrfs (not recalled)

- A new area starts with `TIMELINE_CREATE=yes`, keeps the newest 50 snapshots and never cleans one
  younger than 30 minutes.
- A plain directory (not a subvolume) fails with a clear message and registers nothing.
- **Removing an area deletes its snapshots.** snapall does not offer removal: it would need a loud
  warning, and nobody has asked for it. It is used only to undo a setup that failed half way, when
  there are no snapshots yet.

## Consequences

- A newcomer can go from nothing to protected without leaving snapall or knowing a term.
- The window's start-up hint appears only while something *recommended* is unprotected; optional
  areas are never nagged about.
- Without administrator rights the window explains and names the one command to run, instead of failing.
- Not done: stopping protection of an area from inside snapall.
