#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The README's repository snippets, run as written on a real system against the live repositories:
#   1. run the snippet on a clean system        -> the newest release is installed,
#   2. (apt, dnf, zypper) install the previous release from the same repository, then update the
#      way a person does                         -> it reaches the newest release again.
# The repository must already be published. The snippet is cut from the README at its marker.
#
# usage: repo-snippets.sh deb|dnf|zypper|pacman
# env:   SNIPPET_REF  git ref whose README to test (default: main)
set -euo pipefail
family="${1:?usage: $0 deb|dnf|zypper|pacman}"
repo=grenudi/snapall; ref="${SNIPPET_REF:-main}"
fingerprint=79B4B6BD57666623EAD400B20AE79BC1E50DDD5A
work="$(mktemp -d)"; cd "$work"
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }
[ -n "${GITHUB_TOKEN:-}" ] && printf 'header = "Authorization: Bearer %s"\n' "$GITHUB_TOKEN" > "$HOME/.curlrc" && trap 'rm -f "$HOME/.curlrc"' EXIT

# the repository is published by the workflow just before this runs; wait until it answers
for _ in $(seq 1 30); do curl -fsS -o /dev/null https://grenudi.github.io/snapall/key.asc && break; sleep 10; done
curl -fsSL "https://raw.githubusercontent.com/$repo/$ref/README.md" -o README.md
snippet="$(awk -v id="repo-$family" '
  $0 == "<!-- snippet:" id " -->" { want = 1; next }
  want && /^```/ { if (inblock) exit; inblock = 1; next }
  inblock { print }' README.md)"
[ -n "$snippet" ] || fail "no snippet marked repo-$family in the README at $ref"

mapfile -t tags < <(curl -fsS "https://api.github.com/repos/$repo/releases?per_page=2" | grep -o '"tag_name": *"snapall-v[0-9.]*"' | grep -o 'snapall-v[0-9.]*')
latest="${tags[0]#snapall-v}"; previous="${tags[1]#snapall-v}"
echo "family $family: newest $latest, previous $previous"
have() { snapall --version 2>/dev/null | awk '{print $2}' || true; }

out="$(bash -euo pipefail -c "$snippet" < <(yes) 2>&1)" || { echo "$out" | tail -25 >&2; fail "the snippet failed"; }
[ "$(have)" = "$latest" ] || { echo "$out" | tail -15 >&2; fail "after the snippet expected $latest, got '$(have)'"; }
ok "the snippet installs $latest from the repository"
# the README tells people to compare the fingerprint their package manager shows with its own
case "$family" in
  dnf | zypper)
    echo "$out" | tr -d ' ' | grep -qi "${fingerprint}" || echo "$out" | grep -qi "${fingerprint: -8}" \
      || fail "the package manager did not show the signing key's fingerprint, so the README's advice to compare it cannot be followed: $(echo "$out" | tail -12)"
    ok "$family showed the signing key's fingerprint for the user to compare" ;;
esac

case "$family" in
  deb)
    apt-get install -y -q --allow-downgrades "snapall=$previous-1" >/dev/null 2>&1 || fail "installing $previous from the repository failed"
    [ "$(have)" = "$previous" ] || fail "expected $previous, got '$(have)'"
    ok "the previous release ($previous) installs from the repository"
    apt-get update -q >/dev/null 2>&1; apt-get upgrade -y -q >/dev/null 2>&1
    [ "$(have)" = "$latest" ] || fail "apt upgrade did not reach $latest, got '$(have)'" ;;
  dnf)
    dnf downgrade -y -q "snapall-$previous-1" >/dev/null 2>&1 || fail "downgrading to $previous from the repository failed"
    [ "$(have)" = "$previous" ] || fail "expected $previous, got '$(have)'"
    ok "the previous release ($previous) installs from the repository"
    dnf upgrade -y -q >/dev/null 2>&1
    [ "$(have)" = "$latest" ] || fail "dnf upgrade did not reach $latest, got '$(have)'" ;;
  zypper)
    zypper --non-interactive install --oldpackage "snapall=$previous" >/dev/null 2>&1 || fail "installing $previous from the repository failed"
    [ "$(have)" = "$previous" ] || fail "expected $previous, got '$(have)'"
    ok "the previous release ($previous) installs from the repository"
    zypper --non-interactive update >/dev/null 2>&1
    [ "$(have)" = "$latest" ] || fail "zypper update did not reach $latest, got '$(have)'" ;;
  pacman)
    pacman -Si snapall | grep -q "^Repository *: snapall" || fail "snapall does not come from the snapall repository"
    pacman -Syu --noconfirm >/dev/null 2>&1
    [ "$(have)" = "$latest" ] || fail "a system update changed the version: '$(have)'"
    ok "a system update (pacman -Syu) leaves it at $latest, from the snapall repository" ;;
esac
[ "$family" = pacman ] || ok "an ordinary system update reaches the newest release ($latest)"
echo "PASS: the README's $family repository snippet"
