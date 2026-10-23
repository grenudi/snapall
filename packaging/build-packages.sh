#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

# Build the .deb and .rpm from an already-built static release binary.
#
#   cargo build --release --locked --target x86_64-unknown-linux-musl -p snapall
#   packaging/build-packages.sh x86_64-unknown-linux-musl
#
# Needs: cargo-deb, cargo-generate-rpm, python3. Output goes to dist/. CI and the release
# workflow both call this, so the packages that are tested are the packages that ship.
set -euo pipefail

target="${1:?usage: $0 <rust target triple, e.g. x86_64-unknown-linux-musl>}"
cd "$(dirname "$0")/.."

binary="target/$target/release/snapall"
if [ ! -x "$binary" ]; then
  echo "missing $binary; build it first:" >&2
  echo "  cargo build --release --locked --target $target -p snapall" >&2
  exit 1
fi

mkdir -p dist
# The binary contains MIT/Apache/... libraries whose notices must travel with it.
python3 packaging/third-party-licenses.py "$target"
cargo deb --no-build --package snapall --target "$target" --output dist/
cargo generate-rpm --package crates/snapall --target "$target" --output dist/

# The static binary itself, for distributions that package nothing (Alpine, Void, ...).
cp "$binary" "dist/snapall-$target"

# Checksums only for what is published: the .deb and .rpm. (The bare binary stays in dist/ for
# the distro tests; it is not a release asset, and a checksum for a file nobody can download
# makes `sha256sum -c` fail for users.)
(cd dist && sha256sum -- *.deb *.rpm > SHA256SUMS)
echo "built:" && ls -l dist
