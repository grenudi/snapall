# Booting from a snapshot

A bootloader entry for a snapshot is what lets you get back into a system that no longer starts
after an update: you pick an earlier snapshot in the boot menu, boot it, and restore it from there.
snapall does not set this up and does not edit your bootloader
([why](adr/0013-no-automated-boot-menu-setup.md)). `sudo snapall doctor` tells you which of the cases
below you are in. Everything here comes from the maintainers' own documentation, linked at each step.

| Your boot loader | What gives you snapshot entries | Notes |
|---|---|---|
| Limine | [limine-snapper-sync](https://gitlab.com/Zesko/limine-snapper-sync) | Preconfigured on CachyOS. AUR elsewhere on Arch. |
| GRUB | [grub-btrfs](https://github.com/Antynea/grub-btrfs) | Adds a snapshots submenu; no change of bootloader. |
| systemd-boot, others | no maintained tool known to snapall | Restoring files still works; for the system, use a live USB. |

## If you already boot with Limine
Follow the tool's [README](https://gitlab.com/Zesko/limine-snapper-sync) (its Installation section,
and the [Arch wiki](https://wiki.archlinux.org/title/Limine#Snapper_snapshot_integration_for_Btrfs)).
It needs, in its own words:

- a **FAT32 boot partition, 4 GiB or more recommended**: the kernel and initramfs of every bootable
  snapshot are kept there, and NVIDIA and DKMS modules can add over 300 MiB per kernel;
- Limine 8 or newer, `snapper` and `btrfs-progs`;
- a snapper-managed Btrfs layout;
- an initramfs integration (`limine-mkinitcpio-hook` or `limine-dracut-support`).

It cannot make bootable entries for snapshots taken before it was installed. Once it is set up,
snapshots are synced through a snapper plugin the package installs, so the menu follows `snapall`'s
snapshots without anything else running.

## If you boot something else: should you switch to Limine?
Usually not just for this. If you use GRUB, `grub-btrfs` gives you the same without changing the
bootloader. Switching is a real operation on the part of the machine that must work to start it, and
the usual obstacles are:

- **A small boot partition.** Most installs have far less than 4 GiB. Enlarging it means moving or
  resizing partitions, the riskiest step of all.
- **Other operating systems.** Windows and other entries must be added to the new menu by hand.
- **Secure Boot.** The Arch wiki advises securing Limine's configuration and the kernel files too,
  since anyone who can write the FAT32 partition can alter them.
- **Nothing to go back to** if it fails to boot, except a live USB. Have one before you start.

If you decide to switch, use the maintainers' guides, not a script: the
[Arch wiki](https://wiki.archlinux.org/title/Limine) and the tool's README.

## Encrypted disks (LUKS)
The tool's README says it relies on the kernel, initramfs or UKI for LUKS2 and other encryption,
not on bootloader drivers, and keeps the bootable files outside the encrypted partition. So it works
with an encrypted root, and the price is that the kernel and initramfs are not encrypted.

## Restoring from a booted snapshot
On Limine, boot the snapshot from the menu and run `sudo limine-snapper-restore`: it restores the
snapshot **and its matching kernel**. Restoring only the snapshot can leave a system that does not
start, which is why snapall does not restore the system root itself
([ADR 12](adr/0012-restore-and-the-boot-menu.md)).
