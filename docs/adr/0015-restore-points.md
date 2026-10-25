<!--
SPDX-FileCopyrightText: 2026 grenudi

SPDX-License-Identifier: GPL-3.0-or-later
-->

# 15. A restore point is the set of snapshots one action took, and one action puts it back

Status: accepted

## Context

`snapall` and the window's `a` take a snapshot in every area, but nothing tied those snapshots together:
each had its own number in its own area. To go back to "how everything was before I changed my theme"
a person had to find the right snapshot in each area and restore them one by one, and could get one
wrong.

## Decision

1. **Tag the group.** Every snapshot of one action carries the same tag in snapper's own `userdata`
   (`snapall-point=20261025T181500Z`, the time the action started). The tag is read back with the
   listing. Snapshots taken one at a time carry none.
2. **Names are unique.** Two actions in one second would otherwise share a name and become one point;
   a restore's safety snapshots would then be indistinguishable from what they protect. A name is
   checked against the existing points and gets `-2`, `-3`, ... if taken (found, not imagined: it
   happened in the first end-to-end run).
3. **Older snapshots are matched, not tagged**: same description, different areas, within two
   minutes, one per area. They are marked as a guess everywhere they appear.
4. **One action restores a point**: each area is previewed, takes its own safety snapshot, and is put
   back; one area failing does not stop the others, and every outcome is reported. An area that already
   matches is left alone and takes no safety snapshot.
5. **Undoing is the same action.** The safety snapshots are tagged as a restore point of their own
   (`before restoring <point>`), so the way back is `restore-all` on it, in the command line and in the
   window.
6. **The running system is never put back** (ADR 12): it is skipped and said so, with the route that
   does work. Restoring a point restores every other area.

## Verified on real Btrfs

- The tag goes in on `snapper create --userdata` and comes back in `snapper --jsonout list`.
- Two real subvolumes, files ruined and one deleted: one action puts both back, deleted file
  included; the printed undo command puts back exactly the ruined state.

## Consequences

- A point needs snapshots in at least two areas; one area alone has nothing to put back together.
- Restoring is not atomic across areas (they are independent subvolumes); the safety snapshots make the
  whole thing undoable whatever happens part way.
- The window lists and restores points on its own screen (`R`) as well as from a snapshot's restore
  prompt (`u`, then `A`). Not done: a column in the snapshot list marking which snapshots belong to a
  point.
