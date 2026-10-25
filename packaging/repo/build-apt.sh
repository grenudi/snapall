#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build a signed APT repository from .deb files: a flat repository (`deb URL ./`), the form for a
# single project's packages. The packages are copied unchanged, so they stay byte-identical to the
# release assets whose build provenance was attested.
#
# usage: build-apt.sh OUTDIR PACKAGE.deb...
# env:   SIGN_KEY  fingerprint of the signing key, in the keyring at GNUPGHOME
set -euo pipefail
out="${1:?usage: $0 OUTDIR PACKAGE.deb...}"; shift
: "${SIGN_KEY:?SIGN_KEY (the fingerprint of the signing key) is required}"
[ "$#" -gt 0 ] || { echo "no packages given" >&2; exit 1; }

mkdir -p "$out"
cp -- "$@" "$out/"
cd "$out"
rm -f Packages Packages.gz Release Release.gpg InRelease

apt-ftparchive packages . > Packages
gzip -9nk Packages
apt-ftparchive \
  -o APT::FTPArchive::Release::Origin=snapall \
  -o APT::FTPArchive::Release::Label=snapall \
  -o APT::FTPArchive::Release::Description="snapall packages" \
  release . > Release

# Release.gpg (detached) for older apt, InRelease (clearsigned) for current apt.
gpg --batch --yes --local-user "$SIGN_KEY" --armor --detach-sign --output Release.gpg Release
gpg --batch --yes --local-user "$SIGN_KEY" --clearsign --output InRelease Release
echo "apt repository in $out: $(ls -- *.deb | wc -l) packages"
