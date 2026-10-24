#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Every system the install-snippets workflow tests has a snippet in the README, and each parses.
# (Running them needs a release and a real system: .github/workflows/install-snippets.yml.)
set -euo pipefail
cd "$(dirname "$0")/.."
status=0
for id in deb rpm-dnf rpm-zypper arch tarball nix; do
  snippet="$(awk -v id="$id" '
    $0 == "<!-- snippet:" id " -->" { want = 1; next }
    want && /^```/ { if (inblock) exit; inblock = 1; next }
    inblock { print }' README.md)"
  if [ -z "$snippet" ]; then echo "README.md has no snippet marked '$id'" >&2; status=1; continue; fi
  if ! bash -n <<< "$snippet"; then echo "the '$id' snippet is not valid shell" >&2; status=1; continue; fi
  echo "  ok  $id ($(printf '%s\n' "$snippet" | wc -l) lines)"
done
exit "$status"
