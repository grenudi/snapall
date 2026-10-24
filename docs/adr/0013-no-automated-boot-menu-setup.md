# 13. Setting up the boot menu is explained, not automated

Status: accepted. It extends [ADR 12](0012-restore-and-the-boot-menu.md), which decided that snapall
detects a boot-menu tool and does not edit the bootloader. This records why the next step, *setting
the tool up for the user*, was weighed and declined, and what would change that.

## Context
Detecting that a machine could boot its snapshots, and stopping there, leaves the user to do the
setup. The tempting step is to do it for them: on a Btrfs system, install `limine-snapper-sync`, or
even replace the bootloader with Limine, after showing the risks. What the maintainers' own
documentation says the setup needs
(https://gitlab.com/Zesko/limine-snapper-sync, its Requirements and Limitations):

- a **FAT32 boot partition, 4 GiB or more recommended**, because the kernel and initramfs of every
  bootable snapshot are copied there (NVIDIA and DKMS modules can add over 300 MiB per kernel);
  installs commonly have a boot partition of a few hundred MiB to 1 GiB;
- Limine 8 or newer, `snapper` and `btrfs-progs`, an initramfs integration package (from the AUR on
  most distributions) and, for mkinitcpio, a hand edit of `HOOKS` in `/etc/mkinitcpio.conf`;
- a snapper-managed Btrfs layout, and **no bootable entries for snapshots taken before the tool was
  installed**, because the kernels they had are gone;
- for encryption, the tool relies on the kernel and initramfs to unlock the root and keeps the
  bootable files *outside* the encrypted partition: LUKS works, and kernel and initramfs are not
  encrypted.

## Decision
snapall does not install these tools, edit initramfs configuration, resize or create partitions, or
replace a bootloader. It explains, in `docs/boot-menu.md`, what each setup needs and what switching
costs, and `snapall doctor` points there.

The test applied to any automated setup, and why each candidate fails it:

1. **A failure must not leave the machine unbootable or unrecoverable**, or must be undone by
   snapall itself. A bootloader replacement, an ESP resize and an initramfs rebuild can all end in a
   machine that does not start, and snapall has no way to put the previous state back.
2. **CI must run the exact steps, on every target it would run on, and boot the result.** snapall's
   other privileged action (installing snapper) passes this: containers run it for real. Nothing
   here can be tested without booting virtual machines per distribution, bootloader, partition
   layout and encryption setup, and the combinations do not stop growing.
3. **What is not recognised must be left alone.** Dual boot, Secure Boot, a small boot partition,
   LVM, encryption and a layout the tool does not support are the ordinary cases, not the corner
   ones, and each needs a human decision.

The one step that does pass (1) and (2), enabling a service that is already installed, turned out
not to be needed: the tool keeps the menu in step through a snapper plugin whether or not its
daemon runs, which is how a machine that was working got a false warning from an earlier version of
`doctor`. There is nothing left to automate that is both safe and useful.

## Consequences
The user gets accurate requirements and risks before they change anything, from the maintainers'
words rather than snapall's guesses, and keeps the decision. CachyOS, which ships the Btrfs + Limine
setup preconfigured, needs no setup at all; there, `doctor` simply confirms it works.

Revisit if CI gains virtual-machine boot tests for a specific, narrow combination (for example
Arch-based, Limine already installed, boot partition large enough): a setup limited to exactly
what such a test covers, and refusing everything else, would pass the test above. Showing the boot
partition's size against the tool's recommendation in `doctor` is the cheap, read-only first step
and is in the backlog.
