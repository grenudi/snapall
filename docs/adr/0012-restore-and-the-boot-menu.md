# 12. Restoring, and the boot menu: what snapall does and what it leaves to others

Status: accepted. It supersedes one item of ADR 11, "no Btrfs-specific diagnostics", in one narrow
way: snapall now *reads* whether `/` is Btrfs and which bootloader is installed, only to explain.

## Context
Two requests belong together: *restore a snapshot from snapall*, and *put snapshots in the boot menu
automatically*. Both end at the same place, a machine that must still boot afterwards, so the
evidence came before the design.

- `snapper rollback` "is only supported with btrfs and requires a properly configured system"; it
  works by setting the default subvolume (snapper(8), https://man.archlinux.org/man/snapper.8).
  On layouts that mount a named subvolume (`subvol=@` in fstab: Arch, CachyOS, Fedora, Ubuntu) it has
  no effect on the next boot. The change that would fix it (openSUSE/snapper#1133) was closed
  without being merged, and snapper's latest release at the time of writing is 0.13.2.
- Restoring a subvolume that is in use is done from a live system, with the subvolume not mounted
  (https://wiki.archlinux.org/title/Snapper#Restore_snapshot).
- On Limine the kernel and initramfs are on the boot partition, outside Btrfs. Users report a stuck
  boot after restoring only the snapshot, and that `limine-snapper-restore` is the tool that restores
  the snapshot and its matching kernel together
  (https://discuss.cachyos.org/t/update-broke-my-system-and-snapshot-restore-doesnt-seem-to-work/31954).
  `limine-snapper-sync` (https://gitlab.com/Zesko/limine-snapper-sync) and `grub-btrfs`
  (https://github.com/Antynea/grub-btrfs) already put snapshots in the boot menu and are maintained
  by people who know their bootloader.

## Decisions
- **`snapall restore CONFIG ID [PATH]` is `snapper undochange`**, which snapall's port tests run
  against real Btrfs. It restores files and non-root configs. Two rules make that safe to offer:
  the state just before is snapshotted first, and if that fails nothing is restored, so a restore
  can always be undone; and it asks (or needs `--yes`) after showing what it will change
  (`snapall changes`, or `--dry-run`).
- **The system root is never restored whole.** `restore root ID` is refused; single paths of it are
  allowed. What snapall prints instead is the route that works on *this* machine: through
  `limine-snapper-restore` where Limine's tool is active, through the GRUB snapshot menu and the
  Arch wiki procedure for grub-btrfs, otherwise a live USB. A restore that puts back a snapshot but
  not the kernel is the failure that matters here, and it cannot be seen from snapper.
- **snapall does not write bootloader configuration.** A wrong entry leaves a machine that does not
  start, which is the worst thing a safety tool can do; the layouts differ per distribution; the
  maintained tools exist; and it cannot be tested in CI without booting virtual machines.
- **It detects, read-only, and says so.** `doctor` has a `boot-menu` row (a warning, never a
  failure): Btrfs root, which bootloader's files are present, and whether its snapshot tool is
  installed and running. `create` and the TUI show one line when a tool would help (`--no-hints` or
  `SNAPALL_HINTS=off` silences it), and the TUI's restore prompt carries the route above.
  Zero or several bootloaders found is reported as "could not tell", never guessed.
- **`--json` covers every command that prints a result**, including the new ones.

## Consequences
The file locations the detection looks for come from each tool's documentation, not from a run on
each distribution; a miss costs a missing tip and nothing else, because nothing acts on it. If a
run on a real Limine or GRUB machine disagrees, the heuristic in `boot_probe.rs` is wrong and its
test gets a new case.

Revisit this ADR if CI gains virtual-machine tests that boot a restored snapshot under Limine and
GRUB (then writing entries and restoring the root become testable), or if snapper merges rollback
support for named-subvolume layouts.
