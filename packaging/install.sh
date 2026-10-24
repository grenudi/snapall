#!/bin/sh
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Install snapall from this directory (an unpacked release tarball), or remove it again.
#
#   sudo ./install.sh              install under /usr/local
#   sudo ./install.sh --uninstall  remove what it installed
#   PREFIX=/usr sudo -E ./install.sh
#   DESTDIR=/tmp/stage ./install.sh   (stage into a directory, for packagers; no privileges needed)
#
# This file lives in the tarball, next to what it installs, so the README does not have to list
# files that differ from one release to the next. A file this release does not contain is skipped.
set -eu

prefix="${PREFIX:-/usr/local}"
dest="${DESTDIR:-}"
here="$(cd "$(dirname "$0")" && pwd)"

# mode  source (in this directory)  destination (under the prefix): the one list of what is installed
files() {
  cat <<'LIST'
755 snapall                                      bin/snapall
644 man/snapall.1.gz                             share/man/man1/snapall.1.gz
644 completions/snapall.bash                     share/bash-completion/completions/snapall
644 completions/_snapall                         share/zsh/site-functions/_snapall
644 completions/snapall.fish                     share/fish/vendor_completions.d/snapall.fish
644 completions/snapall.nu                       share/nushell/vendor/autoload/snapall.nu
644 desktop/snapall.desktop                      share/applications/snapall.desktop
644 desktop/snapall.svg                          share/icons/hicolor/scalable/apps/snapall.svg
755 filemanagers/snapall-file-menu               bin/snapall-file-menu
644 filemanagers/snapall.kio-servicemenu.desktop share/kio/servicemenus/snapall.desktop
644 filemanagers/snapall.nemo_action             share/nemo/actions/snapall.nemo_action
644 filemanagers/snapall.libfm-action.desktop    share/file-manager/actions/snapall.desktop
644 filemanagers/snapall-nautilus.py             share/nautilus-python/extensions/snapall.py
LIST
}

case "${1:-install}" in
  install | "")
    [ -f "$here/snapall" ] || { echo "install.sh: run it from the unpacked snapall directory (no ./snapall here)" >&2; exit 1; }
    files | while read -r mode from to; do
      [ -f "$here/$from" ] || continue
      install -Dm"$mode" "$here/$from" "$dest$prefix/$to"
    done
    echo "snapall installed under $dest$prefix (remove it with: ./install.sh --uninstall)"
    ;;
  --uninstall | uninstall)
    files | while read -r _mode _from to; do
      rm -f "$dest$prefix/$to"
    done
    echo "snapall removed from $dest$prefix"
    ;;
  *)
    echo "usage: install.sh [--uninstall]" >&2
    exit 2
    ;;
esac
