#!/bin/sh

# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

# Run INSIDE a fresh distro container (as root), with the repo mounted at /w and the static
# binary at /w/dist/snapall-<target>. Proves, on a real distribution, that
# `snapall doctor` notices a missing snapper, names the right fix, installs it with the real
# package manager, and sees it afterwards.
set -eu
fail() { echo "FAIL: $*" >&2; exit 1; }
# Artifacts lose the executable bit; use a private copy.
for candidate in /w/dist/snapall-*-linux-musl; do
  cp "$candidate" /tmp/snapall
done
chmod +x /tmp/snapall
snapall=/tmp/snapall
if [ -n "${EXPECT_VERSION:-}" ]; then
  [ "$("$snapall" --version)" = "snapall $EXPECT_VERSION" ] || fail "expected snapall $EXPECT_VERSION, got: $("$snapall" --version)"
fi

# shellcheck source=/dev/null
. /etc/os-release
echo "== $PRETTY_NAME"

# A fresh container has no package index. Real systems do; give this one a current one the
# way that distribution's own tooling would. (Arch is special: snapall never runs a bare
# database sync, because that is how partial upgrades happen.)
case "$ID" in
  arch) pacman -Sy --noconfirm >/dev/null ;;
esac

command -v snapper >/dev/null 2>&1 && fail "snapper is already installed in this image"

echo "-- before: doctor must fail and name the fix"
if "$snapall" doctor >/tmp/before.txt 2>&1; then
  cat /tmp/before.txt; fail "doctor succeeded without snapper"
fi
cat /tmp/before.txt
grep -q '^FAIL  snapper' /tmp/before.txt || fail "doctor did not report snapper as failed"
grep -q 'fix:' /tmp/before.txt || fail "doctor did not name a fix"

echo "-- doctor --install --yes"
"$snapall" --no-dbus doctor --install --yes

echo "-- after"
snapper --version
"$snapall" --no-dbus doctor | tee /tmp/after.txt
grep -q '^ok    snapper' /tmp/after.txt || fail "doctor does not see the freshly installed snapper"
echo "PASS: $ID"
