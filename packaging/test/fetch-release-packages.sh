#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Download the .deb and .rpm files of the newest releases into a directory: the real packages the
# repository tests and the repositories workflow work from.
# usage: fetch-release-packages.sh DIR [COUNT]   (GITHUB_TOKEN raises the API rate limit)
# env:   RELEASES_JSON_FILE  a list of releases to use instead of asking GitHub (for tests)
#        LIST_ONLY=1         print the package URLs instead of downloading them (for tests)
set -euo pipefail
dir="${1:?usage: $0 DIR [COUNT]}"; count="${2:-2}"
repo="${GITHUB_REPOSITORY:-grenudi/snapall}"
mkdir -p "$dir"; cd "$dir"
auth=(); [ -n "${GITHUB_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")

# GitHub's releases API lists tags sorted as TEXT (0.9.1 comes before 0.10.0), not by version and not
# by date, so "the newest releases" are chosen here, by version.
releases_json() {
  if [ -n "${RELEASES_JSON_FILE:-}" ]; then cat "$RELEASES_JSON_FILE"
  else curl -fsS "${auth[@]}" "https://api.github.com/repos/$repo/releases?per_page=100"; fi
}
newest_tags() { # newest_tags COUNT: the COUNT highest versions among the tags on stdin, highest first
  grep -o '"tag_name": *"snapall-v[0-9.]*"' | grep -o 'snapall-v[0-9.]*' | sed 's/^snapall-v//' \
    | sort -u -t. -k1,1nr -k2,2nr -k3,3nr | head -n "$1" | sed 's/^/snapall-v/'
}

json="$(releases_json)"
pattern="$(printf '%s\n' "$json" | newest_tags "$count" | paste -sd'|' -)"
[ -n "$pattern" ] || { echo "no releases found" >&2; exit 1; }
printf '%s\n' "$json" | grep -oE "https://[^\"]*/download/($pattern)/[^\"]*\.(deb|rpm)" | sort -u > urls
[ -s urls ] || { echo "no packages found in the newest $count releases" >&2; exit 1; }
[ -z "${LIST_ONLY:-}" ] || { cat urls; rm -f urls; exit 0; }
while read -r url; do curl -fsSLO "${auth[@]}" "$url"; done < urls
rm -f urls
ls
