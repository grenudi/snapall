#!/bin/bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# What snapall prints, for a fixed set of commands on a fresh Btrfs fixture: text, JSON, error messages and exit codes, with what
# changes from run to run (dates, numbers, the version) made stable. A refactoring that must not change behaviour is shown not to
# by `check` passing before and after it.
#
#   dev/golden-output.sh record [binary]   write what the program prints to dev/golden/
#   dev/golden-output.sh check  [binary]   run it again and compare; exits 1 on any difference
#
# Needs root and the fixture tools (dev/btrfs-fixture.sh). It resets the fixture first, so that every run starts from the same disk.
# Machine-dependent commands (health, quotas) are left out on purpose: what they print is the host's, not the program's.
set -u
mode=${1:?usage: dev/golden-output.sh record|check [binary]}
here=$(cd "$(dirname "$0")/.." && pwd)
bin=$(realpath "${2:-$here/target/debug/snapall}")
A=snapall_it_a

"$here/dev/btrfs-fixture.sh" down >/dev/null 2>&1
"$here/dev/btrfs-fixture.sh" down >/dev/null 2>&1
rm -f /etc/snapall/groups /var/lib/snapall/journal.jsonl*
"$here/dev/btrfs-fixture.sh" up >/dev/null || { echo "the fixture would not come up"; exit 2; }

out=$([ "$mode" = record ] && echo "$here/dev/golden" || mktemp -d)
rm -rf "$out"; mkdir -p "$out"

stable() {
  sed -E 's/[0-9]{4}-[0-9]{2}-[0-9]{2}[ T][0-9:]{8}/DATE/g; s/"took_ms":[0-9]+/"took_ms":N/g; s/[0-9]+-[0-9]+-[a-z]/CHANGE/g; s/#[0-9]+/#N/g; s/"id":[0-9]+/"id":N/g; s/"point":[0-9]+/"point":N/g; s/snapall (0\.[0-9]+\.[0-9]+)/snapall VERSION/g; s/snapall@0\.[0-9]+\.[0-9]+/snapall@VERSION/g'
}

while IFS= read -r command; do
  name=$(echo "$command" | tr ' /' '__')
  { "$bin" --no-dbus $command </dev/null 2>&1; echo "exit: $?"; } | stable > "$out/$name.txt"
done <<COMMANDS
list
list --json
snapshots $A
snapshots $A --json
groups
groups --json
limits $A
limits $A --json
activity --json
points
points --json
contract
--help
create --dry-run
create --dry-run --json
delete $A 99999 --destroy
limits nowhere
groups add x
restore $A 99999 --yes
history $A /nonexistent
COMMANDS

if [ "$mode" = record ]; then
  echo "recorded $(ls "$out" | wc -l) commands in dev/golden/"
else
  if diff -r "$here/dev/golden" "$out"; then echo "identical: $(ls "$out" | wc -l) commands print exactly what they printed when recorded"; rm -rf "$out"; else echo "DIFFERENT: see above"; rm -rf "$out"; exit 1; fi
fi
