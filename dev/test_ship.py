#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""The release policy of dev/ship.py, which decides when a merge releases. A wrong answer either floods users with versions or
never releases, and nothing else would notice."""
import importlib.util, os

spec = importlib.util.spec_from_file_location("ship", os.path.join(os.path.dirname(__file__), "ship.py"))
ship = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ship)
why = ship.why_release

CASES = [
    ("a few changes wait", why(3, 0.2, ["crates/snapall/src/main.rs"], set()), None),
    ("nothing is waiting, however old", why(0, 30, [], set()), None),
    ("enough have piled up", why(ship.RELEASE_AFTER, 0.1, ["docs/x.md"], set()), f"{ship.RELEASE_AFTER} changes are waiting"),
    ("one short of enough", why(ship.RELEASE_AFTER - 1, 0.1, [], set()), None),
    ("the oldest has waited too long", why(2, 8, [], set()), "the oldest waiting change is 8 days old"),
    ("just under the age limit", why(2, 6.9, [], set()), None),
    ("packaging changed", why(1, 0, ["packaging/arch/PKGBUILD"], set()), "it touches how releases are published, which only a release proves"),
    ("a release workflow changed", why(1, 0, [".github/workflows/repos.yml"], set()), "it touches how releases are published, which only a release proves"),
    ("an ordinary workflow changed", why(1, 0, [".github/workflows/ci.yml"], set()), None),
    ("a release was asked for", why(1, 0, [], {"--release"}), "a release was asked for"),
]
failed = [(name, got, want) for name, got, want in CASES if got != want]
for name, got, want in failed:
    print(f"WRONG  {name}: got {got!r}, wanted {want!r}")
print(f"{len(CASES) - len(failed)} of {len(CASES)} release decisions are as they should be")
raise SystemExit(1 if failed else 0)
