#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

# Throw-away Btrfs loopback volume with two snapper configs, for the real-snapper tests.
# Needs root, loop devices, btrfs-progs and snapper.   usage: dev/btrfs-fixture.sh up|down
set -euo pipefail

dir="${SNAPALL_FIXTURE_DIR:-/tmp/snapall-fixture}"
configs=(snapall_it_a snapall_it_b)

case "${1:-}" in
  up)
    mkdir -p "$dir/mnt"
    truncate -s 512M "$dir/disk.img"
    mkfs.btrfs -q -f "$dir/disk.img"
    mount -o loop "$dir/disk.img" "$dir/mnt"
    for cfg in "${configs[@]}"; do
      btrfs subvolume create "$dir/mnt/$cfg" >/dev/null
      snapper --no-dbus -c "$cfg" create-config "$dir/mnt/$cfg"
    done
    echo "fixture up: ${configs[*]}"
    ;;
  down)
    # Every config whose volume is inside the fixture goes, not only the two it creates: a test or a
    # person may have added more (a `home` config on a nested subvolume, say), and one left behind
    # points at a path that no longer exists and shows up as an error in every later run.
    mapfile -t configs < <({ printf '%s\n' "${configs[@]}"; grep -ls "^SUBVOLUME=\"$dir/" /etc/snapper/configs/* 2>/dev/null | xargs -r -n1 basename; } | sort -u)
    for cfg in "${configs[@]}"; do
      # snapper cannot delete a config whose volume is no longer mounted (a fixture left behind
      # by an interrupted run), and would leave it registered: so remove the registration
      # itself, which is only a file and a name in a list.
      if ! snapper --no-dbus -c "$cfg" delete-config 2>/dev/null; then
        rm -f "/etc/snapper/configs/$cfg"
        for list in /etc/default/snapper /etc/conf.d/snapper /etc/sysconfig/snapper; do
          [ -f "$list" ] && sed -i "s/\\b$cfg\\b//g; s/SNAPPER_CONFIGS=\" */SNAPPER_CONFIGS=\"/; s/ *\"$/\"/" "$list"
        done
      fi
    done
    umount "$dir/mnt" 2>/dev/null || true
    # a loop device still attached to the image would hold it (and its disk space) open
    for loop in $(losetup -j "$dir/disk.img" 2>/dev/null | cut -d: -f1); do
      losetup -d "$loop" 2>/dev/null || true
    done
    rm -rf "$dir"
    echo "fixture down"
    ;;
  *)
    echo "usage: $0 up|down" >&2
    exit 2
    ;;
esac
