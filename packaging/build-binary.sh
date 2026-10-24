#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

# Build the static release binary so that the same source gives the same bytes wherever it is built.
#
#   packaging/build-binary.sh x86_64-unknown-linux-musl
#
# What differs between two machines, and what this does about it:
#   - the build directory, the Cargo registry and the toolchain end up in the binary as absolute
#     paths (panic locations, debug info): they are rewritten to /build, /cargo and /rust;
#   - incremental compilation is switched off;
#   - the compiler version is pinned by rust-toolchain.toml, so rustup uses the same one everywhere.
# CI and the release workflow both build through this script, so the binary that is tested is the
# binary that ships, and anyone can rebuild a tag with it and compare (`sha256sum`).
set -euo pipefail

target="${1:?usage: $0 <rust target triple, e.g. x86_64-unknown-linux-musl>}"
cd "$(dirname "$0")/.."

root="$PWD"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
sysroot="$(rustc --print sysroot)"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$root=/build --remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$sysroot=/rust"
export CARGO_INCREMENTAL=0

cargo build --release --locked --target "$target" -p snapall
echo "built target/$target/release/snapall with $(rustc -V)"
