#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Download the .deb and .rpm files of the newest releases into a directory: the real packages the
# repository tests and the repositories workflow work from.
# usage: fetch-release-packages.sh DIR [COUNT]   (GITHUB_TOKEN raises the API rate limit)
set -euo pipefail
dir="${1:?usage: $0 DIR [COUNT]}"; count="${2:-2}"
repo="${GITHUB_REPOSITORY:-grenudi/snapall}"
mkdir -p "$dir"; cd "$dir"
auth=(); [ -n "${GITHUB_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
curl -fsS "${auth[@]}" "https://api.github.com/repos/$repo/releases?per_page=$count" \
  | grep -oE 'https://[^"]*snapall-v[0-9.]+/[^"]*\.(deb|rpm)' | sort -u > urls
[ -s urls ] || { echo "no packages found in the newest $count releases" >&2; exit 1; }
while read -r url; do curl -fsSLO "${auth[@]}" "$url"; done < urls
rm -f urls
ls
