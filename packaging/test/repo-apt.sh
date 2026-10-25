#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The APT repository format, against real apt: build a repository from released .deb files with a
# throwaway key, serve it, point apt at it, and check that
#   - it installs an older version and upgrades to the newer one,
#   - a package modified after signing is refused,
#   - a repository signed by another key is refused,
#   - edited metadata is refused.
# Needs root (or sudo), apt, apt-utils and gnupg, and two .deb files of different versions.
# usage: repo-apt.sh OLDER.deb NEWER.deb
set -euo pipefail
older="${1:?usage: $0 OLDER.deb NEWER.deb}"; newer="${2:?usage: $0 OLDER.deb NEWER.deb}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
SUDO=""; [ "$(id -u)" -eq 0 ] || SUDO=sudo
work="$(mktemp -d)"; port=$((20000 + RANDOM % 20000))
keyring=/etc/apt/keyrings/snapall-repo-test.asc; list=/etc/apt/sources.list.d/snapall-repo-test.list
cleanup() {
  [ -n "${server:-}" ] && kill "$server" 2>/dev/null || true
  $SUDO rm -f "$keyring" "$list"; $SUDO apt-get remove -y -q snapall >/dev/null 2>&1 || true
  $SUDO apt-get update -q >/dev/null 2>&1 || true; rm -rf "$work"
}
trap cleanup EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }
version_of() { dpkg-deb -f "$1" Version; }
installed() { dpkg-query -W -f='${Version}' snapall 2>/dev/null || true; }

genkey() { # genkey HOME NAME -> prints the fingerprint
  GNUPGHOME="$1" gpg --batch --quiet --passphrase '' --quick-generate-key "$2 <test@example.invalid>" rsa2048 sign never 2>/dev/null
  GNUPGHOME="$1" gpg --batch --list-keys --with-colons "$2" | awk -F: '/^fpr/{print $10; exit}'
}
mkdir -p "$work/gpg" "$work/other" "$work/site"; chmod 700 "$work/gpg" "$work/other"
fpr="$(genkey "$work/gpg" "snapall test repo")"
other="$(genkey "$work/other" "somebody else")"
GNUPGHOME="$work/gpg" gpg --batch --armor --export "$fpr" > "$work/site/key.asc"
GNUPGHOME="$work/other" gpg --batch --armor --export "$other" > "$work/other.asc"

GNUPGHOME="$work/gpg" SIGN_KEY="$fpr" bash "$root/packaging/repo/build-apt.sh" "$work/site/apt" "$older" "$newer" >/dev/null
(cd "$work/site" && python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1) & server=$!
sleep 1
$SUDO install -d -m 0755 /etc/apt/keyrings
point() { # point KEYFILE: trust this key for the test repository
  $SUDO install -m 0644 "$1" "$keyring"
  echo "deb [signed-by=$keyring] http://127.0.0.1:$port/apt ./" | $SUDO tee "$list" >/dev/null
}
update() { $SUDO apt-get update -o Dir::Etc::sourcelist="$list" -o Dir::Etc::sourceparts=/dev/null -o APT::Get::List-Cleanup=0 2>&1; }

point "$work/site/key.asc"
out="$(update)" || fail "apt-get update failed on a correctly signed repository: $out"
echo "$out" | grep -qiE "^(W|E):" && fail "apt warned about a correctly signed repository: $out"
ok "a repository signed by its key is accepted without warnings"

old_v="$(version_of "$older")"; new_v="$(version_of "$newer")"
$SUDO apt-get install -y -q --allow-downgrades "snapall=$old_v" >/dev/null 2>&1 || fail "installing $old_v from the repository failed"
[ "$(installed)" = "$old_v" ] || fail "expected $old_v installed, got '$(installed)'"
ok "the older version ($old_v) installs from the repository"
$SUDO apt-get install -y -q --only-upgrade snapall >/dev/null 2>&1 || fail "upgrading from the repository failed"
[ "$(installed)" = "$new_v" ] || fail "expected the upgrade to reach $new_v, got '$(installed)'"
ok "an ordinary upgrade reaches the newer version ($new_v)"
$SUDO apt-get remove -y -q snapall >/dev/null 2>&1

# a package changed after signing. apt reuses a cached download whose checksum matches the signed
# index, which would install the pristine file and never look at the tampered one (a machine that
# keeps its downloads, such as a CI runner, does exactly that), so the cache is emptied first: the
# tampered file has to be fetched.
$SUDO apt-get clean
cp "$work/site/apt/$(basename "$newer")" "$work/pristine.deb"
printf 'x' >> "$work/site/apt/$(basename "$newer")"
if $SUDO apt-get install -y -q "snapall=$new_v" >"$work/out" 2>&1; then fail "a modified package must be refused"; fi
[ -z "$(installed)" ] || fail "the modified package was installed anyway"
# apt says "File has unexpected size" when the length changed and "Hash Sum mismatch" when only the content did
grep -qiE "unexpected size|hash sum mismatch|size mismatch" "$work/out" || fail "the refusal should be about the package differing from the signed metadata: $(cat "$work/out")"
ok "a package modified after signing is refused"
cp "$work/pristine.deb" "$work/site/apt/$(basename "$newer")"

# What apt says when a signature is the problem, whichever way it is worded for this version.
sig_complaint='NO_PUBKEY|not signed|couldn.t be verified|isn.t valid|BADSIG|public key is not available'

# a repository signed by someone else
point "$work/other.asc"
out="$(update || true)"
echo "$out" | grep -qiE "$sig_complaint" || fail "a repository signed by a key we do not trust must be refused for its signature, apt said: $out"
ok "a repository whose signature is not by the trusted key is refused ($(echo "$out" | grep -iE "$sig_complaint" | head -1 | cut -c1-90))"
point "$work/site/key.asc"
out="$(update || true)"; echo "$out" | grep -qiE "$sig_complaint" && fail "restoring the right key should make the repository valid again: $out"

# edited metadata
cp "$work/site/apt/InRelease" "$work/InRelease.good"
sed -i 's/^Label: snapall/Label: tampered/' "$work/site/apt/InRelease"
out="$(update || true)"
echo "$out" | grep -qiE "$sig_complaint" || fail "edited metadata must be refused for its signature, apt said: $out"
ok "metadata edited after signing is refused ($(echo "$out" | grep -iE "$sig_complaint" | head -1 | cut -c1-90))"
echo "PASS: the APT repository format"
