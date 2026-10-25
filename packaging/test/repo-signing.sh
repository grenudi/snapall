#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The signing path as the workflow runs it, with a throwaway key: with-key.sh imports the secret and
# checks its fingerprint, the build scripts sign, check-site.sh verifies, and check-site.sh refuses a
# site that is tampered with or signed by another key.
# usage: repo-signing.sh PACKAGE.deb|PACKAGE.rpm...
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }
debs=(); rpms=()
for p in "$@"; do case "$p" in *.deb) debs+=("$p");; *.rpm) rpms+=("$p");; esac; done
[ "${#debs[@]}" -gt 0 ] && [ "${#rpms[@]}" -gt 0 ] || { echo "usage: $0 PACKAGE.deb|PACKAGE.rpm... (both kinds)" >&2; exit 2; }

# a copy of the scripts whose "committed" public key is a throwaway one, as check-site.sh reads it
cp -r "$root/packaging/repo" "$work/repo"
genkey() { GNUPGHOME="$1" gpg --batch --quiet --passphrase '' --quick-generate-key "$2 <t@example.invalid>" rsa2048 sign never 2>/dev/null
           GNUPGHOME="$1" gpg --batch --list-keys --with-colons | awk -F: '/^fpr/{print $10; exit}'; }
mkdir -p "$work/k1" "$work/k2"; chmod 700 "$work/k1" "$work/k2"
fpr="$(genkey "$work/k1" "repo key")"; other="$(genkey "$work/k2" "someone else")"
GNUPGHOME="$work/k1" gpg --batch --armor --export "$fpr" > "$work/repo/snapall-repo.asc"
GNUPGHOME="$work/k1" gpg --batch --armor --export-secret-keys "$fpr" > "$work/secret.asc"
GNUPGHOME="$work/k2" gpg --batch --armor --export-secret-keys "$other" > "$work/other-secret.asc"

# the wrapper: right key works and removes the secret from the command's environment; wrong key refuses
REPO_SIGNING_KEY="$(cat "$work/secret.asc")" FINGERPRINT="$fpr" bash "$work/repo/with-key.sh" \
  bash -c 'test -z "${REPO_SIGNING_KEY:-}" && test -n "$SIGN_KEY" && test -d "$GNUPGHOME"' || fail "the command should get SIGN_KEY and no copy of the secret"
ok "with-key: the command gets the key's fingerprint and no copy of the secret"
if REPO_SIGNING_KEY="$(cat "$work/other-secret.asc")" FINGERPRINT="$fpr" bash "$work/repo/with-key.sh" true 2>"$work/err"; then fail "a secret that is not the expected key must be refused"; fi
grep -q "not the expected" "$work/err" || fail "the refusal should say the key is not the expected one: $(cat "$work/err")"
ok "with-key: a different key in the secret is refused before anything is signed"

# build and check the site
REPO_SIGNING_KEY="$(cat "$work/secret.asc")" FINGERPRINT="$fpr" bash "$work/repo/with-key.sh" bash -c "
  bash '$work/repo/build-apt.sh' '$work/site/apt' ${debs[*]} && bash '$work/repo/build-rpm.sh' '$work/site/rpm' ${rpms[*]}" >/dev/null
cp "$work/repo/snapall-repo.asc" "$work/site/key.asc"
bash "$work/repo/check-site.sh" "$work/site" "$fpr" >/dev/null || fail "a correctly built site must pass the check"
ok "check-site: a correctly built and signed site passes"

# a package swapped after signing
victim="$(ls "$work"/site/apt/*.deb | head -1)"; cp "$victim" "$work/victim.bak"; printf x >> "$victim"
if bash "$work/repo/check-site.sh" "$work/site" "$fpr" >/dev/null 2>&1; then fail "a modified package must fail the check"; fi
cp "$work/victim.bak" "$victim"; ok "check-site: a package modified after signing fails"

# metadata signed by another key
REPO_SIGNING_KEY="$(cat "$work/other-secret.asc")" FINGERPRINT="$other" bash "$work/repo/with-key.sh" \
  gpg --batch --yes --local-user "$other" --armor --detach-sign --output "$work/site/apt/Release.gpg" "$work/site/apt/Release" 2>/dev/null
if bash "$work/repo/check-site.sh" "$work/site" "$fpr" >/dev/null 2>&1; then fail "a signature by another key must fail the check"; fi
ok "check-site: a signature by another key fails"

# a site that serves a different key than the committed one
cp "$work/repo/snapall-repo.asc" "$work/site/key.asc"; echo "# changed" >> "$work/site/key.asc"
if bash "$work/repo/check-site.sh" "$work/site" "$fpr" >/dev/null 2>&1; then fail "serving another key must fail the check"; fi
ok "check-site: serving a key other than the committed one fails"
echo "PASS: the signing path"
