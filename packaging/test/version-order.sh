#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The scripts that test upgrades and build the package repositories must choose "the newest releases"
# by VERSION. GitHub's releases API does not: it lists tags sorted as text, so 0.10.0 comes after
# 0.7.2 and 0.9.1 comes first. Taking the API's first entries once put 0.9.1 ahead of 0.10.0: the
# install tests expected the wrong version, and the apt repository was published without the newest
# release. This runs the three scripts offline against a list in that real, text-sorted order.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }

# the order GitHub actually returned them in (observed), with each release's packages
{
  echo '['
  for v in 0.9.1 0.9.0 0.8.0 0.7.2 0.10.0 0.2.0; do
    echo "  {\"tag_name\": \"snapall-v$v\", \"assets\": ["
    for f in "snapall_${v}-1_amd64.deb" "snapall-${v}-1.x86_64.rpm" "snapall-${v}.tar.gz"; do
      echo "    {\"browser_download_url\": \"https://github.com/grenudi/snapall/releases/download/snapall-v$v/$f\"},"
    done
    echo "  ]},"
  done
  echo ']'
} > "$work/releases.json"

export RELEASES_JSON_FILE="$work/releases.json" SELECT_ONLY=1

got="$(bash "$here/readme-snippets.sh" deb | tail -n1)"
[ "$got" = "0.10.0 0.9.1" ] || fail "readme-snippets.sh chose '$got', expected the newest 0.10.0 and the previous 0.9.1"
ok "the install-and-upgrade test treats 0.10.0 as newer than 0.9.1"

got="$(bash "$here/repo-snippets.sh" deb | tail -n1)"
[ "$got" = "0.10.0 0.9.1" ] || fail "repo-snippets.sh chose '$got', expected 0.10.0 and 0.9.1"
ok "the repository test treats 0.10.0 as newer than 0.9.1"

unset SELECT_ONLY
export LIST_ONLY=1
versions() { sed -E 's#.*/download/snapall-v([0-9.]+)/.*#\1#' | sort -u -t. -k1,1nr -k2,2nr -k3,3nr | paste -sd' ' -; }
got="$(bash "$here/fetch-release-packages.sh" "$work/out" 3 | versions)"
[ "$got" = "0.10.0 0.9.1 0.9.0" ] || fail "the repositories would be built from '$got', expected 0.10.0 0.9.1 0.9.0"
ok "the repositories are built from the three newest versions, 0.10.0 included"
got="$(bash "$here/fetch-release-packages.sh" "$work/out" 2 | versions)"
[ "$got" = "0.10.0 0.9.1" ] || fail "two newest: got '$got'"
ok "and the two newest are 0.10.0 and 0.9.1"
bash "$here/fetch-release-packages.sh" "$work/out" 3 | grep -q 'tar.gz' && fail "only .deb and .rpm belong in the repositories"
ok "only packages that belong in a repository are listed"
echo "PASS: releases are chosen by version"
