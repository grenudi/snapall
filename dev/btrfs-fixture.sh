#!/usr/bin/env bash
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
    for cfg in "${configs[@]}"; do
      snapper --no-dbus -c "$cfg" delete-config 2>/dev/null || true
    done
    umount "$dir/mnt" 2>/dev/null || true
    rm -rf "$dir"
    echo "fixture down"
    ;;
  *)
    echo "usage: $0 up|down" >&2
    exit 2
    ;;
esac
