#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build a signed RPM repository from .rpm files, one directory per architecture (`$basearch`), for
# dnf and zypper. The packages are copied unchanged, so they stay byte-identical to the release
# assets whose build provenance was attested; what is signed is the repository metadata, which holds
# each package's checksum, so a modified package is caught (`repo_gpgcheck=1`).
#
# usage: build-rpm.sh OUTDIR PACKAGE.rpm...
# env:   SIGN_KEY  fingerprint of the signing key, in the keyring at GNUPGHOME
set -euo pipefail
out="${1:?usage: $0 OUTDIR PACKAGE.rpm...}"; shift
: "${SIGN_KEY:?SIGN_KEY (the fingerprint of the signing key) is required}"
[ "$#" -gt 0 ] || { echo "no packages given" >&2; exit 1; }

for package in "$@"; do
  arch="$(rpm -qp --queryformat '%{ARCH}' -- "$package" 2>/dev/null)"
  [ -n "$arch" ] || { echo "cannot read the architecture of $package" >&2; exit 1; }
  mkdir -p "$out/$arch"
  cp -- "$package" "$out/$arch/"
done
for dir in "$out"/*/; do
  createrepo_c --quiet --no-database "$dir"
  rm -f "$dir/repodata/repomd.xml.asc"
  gpg --batch --yes --local-user "$SIGN_KEY" --armor --detach-sign --output "$dir/repodata/repomd.xml.asc" "$dir/repodata/repomd.xml"
  echo "rpm repository $(basename "$dir"): $(ls "$dir"/*.rpm | wc -l) packages"
done
