#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The file-manager integration, tested without a desktop:
#   1. each entry has what its file manager's documentation requires, and runs the launcher on one file;
#   2. the launcher runs the right command in each terminal it knows, passes any file name through
#      intact, and tells you when it finds no terminal;
#   3. the Nautilus extension offers the entry for one local file and nothing else.
# usage: filemanagers.sh [DIR]   (DIR defaults to packaging/filemanagers)
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1   # importing the extension must not leave files in the source tree
dir="${1:-$(dirname "$0")/../filemanagers}"
dir="$(cd "$dir" && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }

# ---- 1. the entries ----------------------------------------------------------------------------
python3 - "$dir" <<'PYEOF'
import configparser, sys, pathlib
d = pathlib.Path(sys.argv[1])

def load(name):
    p = configparser.RawConfigParser(delimiters=("=",), comment_prefixes=("#",), interpolation=None, strict=True)
    p.optionxform = str  # keys are case sensitive in all of these formats
    p.read(d / name, encoding="utf-8")
    return p

def exec_ok(value, placeholder):
    words = value.split()
    assert words[0] == "snapall-file-menu", f"runs the launcher, got: {value}"
    assert words[1:] == [placeholder], f"passes exactly one file as {placeholder}, got: {value}"

# Dolphin and other KDE file managers (develop.kde.org: Type=Service, MimeType, Actions, Desktop Action)
k = load("snapall.kio-servicemenu.desktop")
e = k["Desktop Entry"]
assert e["Type"] == "Service"
assert "application/octet-stream" in e["MimeType"].split(";"), "all files"
assert "inode/directory" not in e["MimeType"], "not folders: a folder has no single file to show"
actions = [a for a in e["Actions"].split(";") if a]
assert actions, "an action is listed"
for a in actions:
    section = k[f"Desktop Action {a}"]
    assert section["Name"] and section["Icon"] == "snapall"
    exec_ok(section["Exec"], "%f")
print("  ok  Dolphin: a servicemenu with one action for all files, running the launcher on %f")

# Nemo (the keys its own sample action documents)
n = load("snapall.nemo_action")["Nemo Action"]
assert n["Name"] and n["Selection"] == "s", "one selected file"
assert "nodirs" in n["Extensions"].split(";"), "not folders"
assert n["Quote"] == "double", "a path with spaces stays one argument"
exec_ok(n["Exec"], "%F")
assert "snapall" in n["Dependencies"].split(";"), "hidden where snapall is not installed"
print("  ok  Nemo: one selected file, quoted, hidden without snapall")

# PCManFM and PCManFM-Qt (libfm actions: Type=Action, Profiles, X-Action-Profile)
l = load("snapall.libfm-action.desktop")
e = l["Desktop Entry"]
assert e["Type"] == "Action" and e["Name"]
profiles = [p for p in e["Profiles"].split(";") if p]
assert profiles
for p in profiles:
    section = l[f"X-Action-Profile {p}"]
    assert "all/allfiles" in section["MimeTypes"].split(";"), "files, not folders"
    exec_ok(section["Exec"], "%f")
print("  ok  PCManFM: an action with a profile for all files, running the launcher on %f")
PYEOF

# ---- 2. the launcher, with fake terminals --------------------------------------------------------
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
export RECORD="$work/rec"   # where the fake terminals record what they were given
bin="$work/bin"; mkdir -p "$bin"
# Only the tools the launcher itself needs, so that no real terminal on this machine is found.
for tool in sh basename; do ln -s "$(command -v $tool)" "$bin/$tool"; done
fake() { # fake NAME: records its arguments, NUL-separated, and does not run them
  printf '#!/bin/sh\nprintf "%%s\\0" "$0" "$@" > "$RECORD"\n' > "$bin/$1"; chmod +x "$bin/$1"
}
args_of() { tr '\0' '\n' < "$RECORD"; }
run() { PATH="$bin" TERMINAL="${TERMINAL:-}" "$(command -v sh)" "$dir/snapall-file-menu" "$1"; }
weird="/tmp/my notes/it's \"quoted\" & \$odd.txt"

expect() { # expect LABEL 'argv words before the sh -c' ...
  local label=$1; shift
  rm -f "$work/rec"
  run "$weird" || fail "$label: the launcher failed"
  local got; got="$(args_of | sed -n '2,$p' | head -n "$#" | tr '\n' ' ')"
  local want="$* "
  [ "$got" = "$want" ] || fail "$label: expected the terminal to get '$want' first, got '$got'"
  [ "$(args_of | tail -n 1)" = "$weird" ] || fail "$label: the file name did not arrive as one intact argument: $(args_of | tail -n 1)"
  args_of | grep -q 'sudo snapall tui --file "\$1"' || fail "$label: the command run is not 'sudo snapall tui --file'"
  ok "$label"
}

fake xdg-terminal-exec; expect "xdg-terminal-exec runs the command itself" sh -c
rm "$bin/xdg-terminal-exec"
for pair in "konsole:-e" "alacritty:-e" "xterm:-e" "gnome-terminal:--" "ptyxis:--" "xfce4-terminal:-x"; do
  t=${pair%%:*}; flag=${pair#*:}; fake "$t"; expect "$t gets $flag" "$flag" sh -c; rm "$bin/$t"
done
for t in kitty foot; do fake "$t"; expect "$t gets the command with no flag" sh -c; rm "$bin/$t"; done
fake wezterm; expect "wezterm gets start --" start -- sh -c; rm "$bin/wezterm"

fake konsole; fake kitty
TERMINAL=kitty expect "\$TERMINAL is preferred to the built-in list" sh -c
rm "$bin/konsole" "$bin/kitty"

rm -f "$work/rec"
if run "/x" 2>"$work/err"; then fail "with no terminal the launcher must fail"; fi
grep -q "xdg-terminal-exec" "$work/err" || fail "no terminal: the message should say what to install"
ok "no terminal: it fails and says what to install"

# the command inside the terminal: really run, with a fake sudo, for success and for failure
printf '#!/bin/sh\nprintf "%%s\\n" "$@" > "$RECORD"\nexit "${SUDO_EXIT:-0}"\n' > "$bin/sudo"; chmod +x "$bin/sudo"
printf '#!/bin/sh\nexec "$@"\n' > "$bin/xdg-terminal-exec"; chmod +x "$bin/xdg-terminal-exec"
printf '#!/bin/sh\ntrue\n' > "$bin/read"; chmod +x "$bin/read"
RECORD="$work/sudo" PATH="$bin" SUDO_EXIT=0 sh "$dir/snapall-file-menu" "$weird" >/dev/null
[ "$(cat "$work/sudo")" = "$(printf 'snapall\ntui\n--file\n%s' "$weird")" ] || fail "sudo did not get: snapall tui --file <the file>"
ok "inside the terminal: sudo snapall tui --file FILE, with the name intact"
out="$(RECORD="$work/sudo" PATH="$bin" SUDO_EXIT=1 sh "$dir/snapall-file-menu" "$weird" </dev/null 2>&1 || true)"
echo "$out" | grep -q "could not open this file" || fail "a failure must be shown, not lost when the terminal closes"
ok "inside the terminal: a failure is shown and waits for Enter instead of vanishing"

# ---- 3. the Nautilus extension, against a stand-in for GNOME's API ----------------------------------
python3 - "$dir" <<'PYEOF'
import sys, types, importlib.util, pathlib
d = pathlib.Path(sys.argv[1])
calls = []

class File:  # what Nautilus.FileInfo offers
    def __init__(self, path, scheme="file", directory=False):
        self.path, self.scheme, self.directory = path, scheme, directory
    def get_uri_scheme(self): return self.scheme
    def is_directory(self): return self.directory
    def get_location(self): return types.SimpleNamespace(get_path=lambda: self.path)

class MenuItem:
    def __init__(self, name, label, tip): self.name, self.label, self.tip, self.handlers = name, label, tip, []
    def connect(self, signal, fn, *data): self.handlers.append((signal, fn, data))

nautilus = types.ModuleType("Nautilus"); nautilus.MenuProvider = type("MenuProvider", (), {}); nautilus.MenuItem = MenuItem
gobject = types.ModuleType("GObject"); gobject.GObject = type("GObject", (), {})
repo = types.ModuleType("gi.repository"); repo.Nautilus, repo.GObject = nautilus, gobject
gi = types.ModuleType("gi"); gi.repository = repo
versions = []
gi.require_version = lambda name, version: versions.append((name, version))
sys.modules.update({"gi": gi, "gi.repository": repo})
import shutil, subprocess
shutil.which = lambda name: "/usr/bin/" + name
subprocess.Popen = lambda argv, **kw: calls.append((argv, kw))

spec = importlib.util.spec_from_file_location("snapall_nautilus", d / "snapall-nautilus.py")
mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)
ext = mod.SnapallMenu()
assert versions == [("Nautilus", "4.0")], versions

one = File("/home/me/a b.txt")
for args, label in (((None, [one]), "API 3.0 (window, files)"), (([one],), "API 4.0 (files)")):
    items = ext.get_file_items(*args)
    assert len(items) == 1 and items[0].label == "Restore from snapshots", label
    print(f"  ok  Nautilus {label}: one entry for one local file")
(_, fn, data), = items[0].handlers
fn(items[0], *data)
assert calls == [(["snapall-file-menu", "/home/me/a b.txt"], {"start_new_session": True})], calls
print("  ok  Nautilus: activating runs the launcher with the path, detached")
assert ext.get_file_items([one, File("/x")]) == [], "two files: no entry"
assert ext.get_file_items([File("/home/me/dir", directory=True)]) == [], "a folder: no entry"
assert ext.get_file_items([File("/net/f", scheme="smb")]) == [], "a remote file: no entry"
assert ext.get_file_items([]) == []
shutil.which = lambda name: None
assert ext.get_file_items([one]) == [], "without the launcher: no entry"
print("  ok  Nautilus: nothing offered for several files, a folder, a remote file or without snapall")
PYEOF
echo "PASS: the file-manager integration"
