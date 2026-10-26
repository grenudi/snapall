<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# 16. Health and Activity: one place to see whether protection is working

Status: accepted (the foundation is built; the window's tabs follow)

## Context

snapall can take, list, protect and restore, but it cannot answer the questions a person has once it
is set up: *Is it working? How much room do the snapshots take? Will old ones be cleaned up, or will
they fill the disk? What did snapall just do?* Those answers are scattered over `df`, `systemctl`,
`snapper get-config` and the boot loader, in words nobody has been told.

## What can be known (measured on real Btrfs, not assumed)

- **Space held by snapshots is only measurable with Btrfs quotas.** Without them `btrfs qgroup show`
  fails (*quotas not enabled*) and snapper's `used-space` is empty. With them, snapper reports each
  snapshot's exclusive bytes exactly. So the screen says *not measured* (and why) rather than guess.
- **Retention is per area** (`NUMBER_LIMIT` 50, `NUMBER_LIMIT_IMPORTANT` 10, `NUMBER_MIN_AGE` 1800 s,
  and `SPACE_LIMIT` / `FREE_LIMIT`, which only act with quotas), and it only happens if snapper's
  cleanup timer is enabled **and running**.
- Disk size, use and free, and the state of each service, are ordinary system facts.

## Decision

### Names (what a person reads)

Four tabs: **Snapshots** (today's screen), **Restore points**, **Health**, **Activity**.
Health has four sections: **Space**, **Automatic cleanup**, **Services**, **Boot menu**. Its first line
is a verdict (*All good* or *N things need attention*) followed by what to do, most urgent first: the
screen is useful before it is read in full. "Metrics" is what it holds; "Health" is what it is for.

### Layout (80x24)

```
 Snapshots | Restore points | Health | Activity            <- tabs (1 to 4, or click)
 ...                                                        <- the tab's content
 41 snapshots | 3.2 GiB | disk 62% | cleanup on | boot ok   <- status bar, every tab, each clickable
 (what the mouse is over, or the last message)
 c create  a all  d delete ...                              <- keys
```

The status bar is on every tab; a click goes to Health with that item's section open and explained. A
prompt never covers the three lines below the content.

### Behaviour

- Every item in Health is selectable; Enter or a click explains it in plain words (what it is, why it
  matters, what to do) and offers the action where there is one that snapall can take (switch on
  cleanup is the first).
- **Activity** is what snapall did and how it went, newest first: the operations of this session from
  the same observer that writes the audit log, with how long each took and the failures kept visible.
  It is not a persistent log; `--log-file` is, and the tab says so.

### Structure (so the code reads like prose)

1. `snapall-core::health` is **pure**: facts in, a report out.
   `assess(&Facts) -> Health { verdict, findings, areas, services, chips }`. All the rules ("disk over
   85%", "cleanup not running", "no snapshot of Home in 45 days") and all their plain-words
   explanations live there, tested without a machine.
2. Facts come through two ports: `SnapshotBackend` (per-area retention settings, each snapshot's
   measured size) and `SystemProbe` (disk and services). A small adapter crate, `snapall-system`,
   implements the second; nothing else knows how `systemctl` or `df` is called.
3. The window gains a `Tab` and one module per tab, each with its own `render` and `on_key`, and
   `model.rs` is split along the same lines. A tab is read as a list of what it shows, not as a
   `match` over every key in the program.
4. The command line gets the same report: `snapall health` (and `--json`), so the screen and the
   command cannot disagree, and the rules are testable on the real fixture.

## Consequences

- Nothing is invented: an unmeasurable figure is shown as unmeasured, with the reason.
- The status bar and the tab share one report, so a chip and its explanation cannot drift apart.
- Not decided here: changing retention from the window (the first action will be switching cleanup on,
  which `setup` already does).
