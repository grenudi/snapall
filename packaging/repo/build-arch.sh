#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build a signed pacman repository from a built package: the package gets a detached signature, and
# the repository database is signed too, so a client can require both (`SigLevel = Required`). Run
# in an Arch container, with the signing key in the keyring at GNUPGHOME. It signs and indexes; it
# never builds or runs anything from the package.
#
# usage: build-arch.sh OUTDIR PACKAGE.pkg.tar.zst
# env:   SIGN_KEY  fingerprint of the signing key
set -euo pipefail
out="${1:?usage: $0 OUTDIR PACKAGE.pkg.tar.zst}"; package="${2:?usage: $0 OUTDIR PACKAGE.pkg.tar.zst}"
: "${SIGN_KEY:?SIGN_KEY (the fingerprint of the signing key) is required}"

mkdir -p "$out"
cp -- "$package" "$out/"
cd "$out"
file="$(basename "$package")"
rm -f snapall.db* snapall.files* "$file.sig"

# pacman wants a binary (not armoured) detached signature next to the package
gpg --batch --yes --local-user "$SIGN_KEY" --detach-sign --output "$file.sig" "$file"
repo-add --sign --key "$SIGN_KEY" snapall.db.tar.zst "$file" >/dev/null

# pacman asks for snapall.db and snapall.db.sig; repo-add makes them symlinks, which a static web
# host does not serve, so they become real files.
for kind in db files; do
  cp --remove-destination "snapall.$kind.tar.zst" "snapall.$kind"
  cp --remove-destination "snapall.$kind.tar.zst.sig" "snapall.$kind.sig"
done
echo "pacman repository in $out: $file"
