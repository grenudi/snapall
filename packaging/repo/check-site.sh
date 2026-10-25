#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Check an assembled repository site before it is published: every signature on it verifies against
# the committed public key (and that key is the expected one), and what should be there is there.
# A site that fails this is never deployed.
#
# usage: check-site.sh SITEDIR EXPECTED-FINGERPRINT
set -euo pipefail
site="${1:?usage: $0 SITEDIR FINGERPRINT}"; expected="${2:?usage: $0 SITEDIR FINGERPRINT}"
here="$(cd "$(dirname "$0")" && pwd)"
export GNUPGHOME; GNUPGHOME="$(mktemp -d)"; chmod 700 "$GNUPGHOME"; trap 'rm -rf "$GNUPGHOME"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }

gpg --batch --quiet --import "$here/snapall-repo.asc" 2>/dev/null
have="$(gpg --batch --list-keys --with-colons | awk -F: '/^fpr/{print $10; exit}')"
[ "$have" = "$expected" ] || fail "the committed public key is $have, expected $expected"
ok "the committed public key is the expected one"
cmp -s "$site/key.asc" "$here/snapall-repo.asc" || fail "the key the site serves is not the committed one"
ok "the site serves that key"

verify() { # verify SIGNATURE [FILE]
  local out; out="$(gpg --batch --verify "$@" 2>&1)" || fail "bad signature on $1: $out"
  echo "$out" | grep -q "${expected: -16}" || fail "$1 is signed by some other key: $out"
}
if [ -d "$site/apt" ]; then
  verify "$site/apt/Release.gpg" "$site/apt/Release"
  gpg --batch --verify "$site/apt/InRelease" >/dev/null 2>&1 || fail "InRelease does not verify"
  grep -q "${expected: -16}" <(gpg --batch --verify "$site/apt/InRelease" 2>&1) || fail "InRelease is signed by another key"
  n="$(ls "$site"/apt/*.deb | wc -l)"; [ "$n" -gt 0 ] || fail "the apt repository holds no packages"
  # every package named in the signed index is present and has the checksum the index states
  python3 - "$site/apt" <<'PYEOF' || fail "the apt index does not match the packages"
import sys, hashlib, pathlib, re
d = pathlib.Path(sys.argv[1])
for block in (d / "Packages").read_text().strip().split("\n\n"):
    f = dict(l.split(": ", 1) for l in block.splitlines() if ": " in l and not l.startswith(" "))
    p = d / f["Filename"].lstrip("./")
    assert hashlib.sha256(p.read_bytes()).hexdigest() == f["SHA256"], p.name
rel = (d / "Release").read_text()
packages = hashlib.sha256((d / "Packages").read_bytes()).hexdigest()
assert packages in rel, "Release does not list Packages' checksum"
PYEOF
  ok "apt: Release, Release.gpg and InRelease verify; the index matches $n package(s)"
fi
for repo in "$site"/rpm/*/; do
  [ -d "$repo" ] || continue
  verify "$repo/repodata/repomd.xml.asc" "$repo/repodata/repomd.xml"
  ok "rpm $(basename "$repo"): repomd.xml verifies ($(ls "$repo"/*.rpm | wc -l) package(s))"
done
for repo in "$site"/arch/*/; do
  [ -d "$repo" ] || continue
  verify "$repo/snapall.db.sig" "$repo/snapall.db"
  for package in "$repo"/*.pkg.tar.zst; do verify "$package.sig" "$package"; done
  ok "pacman $(basename "$repo"): the database and every package verify"
done
echo "PASS: the site's signatures"
