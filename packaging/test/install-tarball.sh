#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The tarball's own installer (packaging/install.sh), on a synthetic unpacked tarball:
#   - it installs every file where the .deb puts it (so the two lists cannot drift apart),
#   - with the right modes, under PREFIX and into DESTDIR,
#   - skips what a release does not contain, and refuses to run from the wrong directory,
#   - and --uninstall removes exactly what was installed.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }

# What the .deb installs (outside the docs), from its own metadata: the other list to compare with.
python3 - "$root" > "$work/deb-list" <<'PYEOF'
import sys, tomllib, pathlib, os
root = pathlib.Path(sys.argv[1])
meta = tomllib.load(open(root / "crates/snapall/Cargo.toml", "rb"))["package"]["metadata"]["deb"]
for src, dest, _mode in meta["assets"]:
    if dest.endswith("/"):
        dest += os.path.basename(src)
    if dest.startswith("usr/share/doc/"):
        continue
    assert dest.startswith("usr/"), dest
    print(dest[len("usr/"):])
PYEOF
sort -o "$work/deb-list" "$work/deb-list"

# A synthetic unpacked tarball: every file the installer knows, containing its own name.
tree="$work/snapall-vTEST"; mkdir -p "$tree"; cp "$root/packaging/install.sh" "$tree/install.sh"
awk '/^LIST$/{exit} f&&NF{print $1, $2, $3} /<<.LIST.$/{f=1}' "$root/packaging/install.sh" > "$work/list"
[ -s "$work/list" ] || fail "could not read the installer's file list"
while read -r _mode from _to; do mkdir -p "$tree/$(dirname "$from")"; echo "$from" > "$tree/$from"; done < "$work/list"

installed() { (cd "$1" && find . -type f | sed 's|^\./||' | sort); }

stage="$work/stage"
DESTDIR="$stage" PREFIX=/usr sh "$tree/install.sh" >/dev/null
awk '{print $3}' "$work/list" | sort > "$work/installer-list"
diff "$work/installer-list" "$work/deb-list" > "$work/diff" || { cat "$work/diff" >&2; fail "the installer and the .deb install different files (< installer, > deb)"; }
ok "the installer's file list is the .deb's, file for file"
[ "$(installed "$stage/usr")" = "$(cat "$work/installer-list")" ] || fail "what was installed is not what the list says"
ok "every listed file is installed, and nothing else"

while read -r mode from to; do
  actual="$(stat -c %a "$stage/usr/$to")"
  [ "$actual" = "$mode" ] || fail "$to has mode $actual, expected $mode"
done < "$work/list"
ok "modes: the binaries are executable, the rest are not"

rm -rf "$stage"; DESTDIR="$stage" sh "$tree/install.sh" >/dev/null
[ -x "$stage/usr/local/bin/snapall" ] || fail "the default prefix is /usr/local"
ok "the default prefix is /usr/local, and DESTDIR stages elsewhere"

# a release that lacks files (an older or newer layout) is installed as far as it goes
partial="$work/partial"; cp -r "$tree" "$partial"; rm -rf "$partial/filemanagers" "$partial/desktop"
rm -rf "$stage"; DESTDIR="$stage" sh "$partial/install.sh" >/dev/null || fail "a tarball without some files must still install"
[ -x "$stage/usr/local/bin/snapall" ] && [ ! -e "$stage/usr/local/share/applications" ] || fail "partial install is wrong"
ok "files this release lacks are skipped, not an error"

# wrong directory
mkdir "$work/empty"; cp "$root/packaging/install.sh" "$work/empty/"
if DESTDIR="$stage" sh "$work/empty/install.sh" 2>"$work/err"; then fail "no ./snapall here: it must refuse"; fi
grep -q "unpacked snapall directory" "$work/err" || fail "the refusal should say why"
ok "run from the wrong directory it refuses and says why"

# uninstall removes exactly what was installed
rm -rf "$stage"; DESTDIR="$stage" PREFIX=/usr sh "$tree/install.sh" >/dev/null
mkdir -p "$stage/usr/bin"; echo keep > "$stage/usr/bin/somebody-elses-tool"
DESTDIR="$stage" PREFIX=/usr sh "$tree/install.sh" --uninstall >/dev/null
[ "$(installed "$stage")" = "usr/bin/somebody-elses-tool" ] || fail "uninstall must remove snapall's files and nothing else: $(installed "$stage")"
ok "--uninstall removes what was installed and leaves other files alone"

DESTDIR="$stage" sh "$tree/install.sh" --bogus 2>/dev/null && fail "an unknown option must be refused"
ok "an unknown option is refused"
echo "PASS: the tarball installer"
