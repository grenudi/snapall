#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Write THIRD-PARTY-LICENSES.md: the notices of every library inside the snapall binary.

The binary is distributed with its dependencies compiled in, and MIT, BSD and Apache-2.0
all require their copyright and license notices to travel with the binary. This collects
them from the actual dependency closure of the `snapall` package (normal dependencies,
for the given target), using the license files each crate ships.

    python3 packaging/third-party-licenses.py [rust-target-triple] [output-file]

Needs `cargo metadata` and the crates' sources to be downloaded (`cargo fetch --locked`).
"""

import hashlib
import json
import os
import re
import subprocess
import sys

TARGET = sys.argv[1] if len(sys.argv) > 1 else "x86_64-unknown-linux-musl"
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, "THIRD-PARTY-LICENSES.md")
LICENSE_FILE = re.compile(r"^(licen[sc]e|copying|unlicen[sc]e|notice)([-_.].*)?$", re.IGNORECASE)


def metadata():
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)


def closure(meta, root_name):
    """Package ids reachable from `root_name` through normal (non-dev, non-build) edges."""
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(p["id"] for p in meta["packages"] if p["name"] == root_name and p["source"] is None)
    seen, stack = set(), [root]
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dep in nodes[current]["deps"]:
            if any(kind["kind"] is None for kind in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    return seen


def license_files(package):
    directory = os.path.dirname(package["manifest_path"])
    found = []
    for name in sorted(os.listdir(directory)):
        path = os.path.join(directory, name)
        if os.path.isfile(path) and LICENSE_FILE.match(name):
            with open(path, encoding="utf-8", errors="replace") as handle:
                found.append((name, handle.read().strip("\n")))
    return found


def main():
    meta = metadata()
    wanted = closure(meta, "snapall")
    packages = sorted(
        (p for p in meta["packages"] if p["id"] in wanted and p["source"] is not None),
        key=lambda p: (p["name"], p["version"]),
    )

    texts = {}  # sha -> (text, [crate labels], {file names})
    missing = []
    rows = []
    for package in packages:
        label = f'{package["name"]} {package["version"]}'
        rows.append((label, package.get("license") or "(none declared)", ", ".join(package.get("authors") or [])))
        files = license_files(package)
        if not files:
            missing.append((label, package.get("license") or "(none declared)", package.get("authors") or []))
        for filename, text in files:
            key = hashlib.sha256(text.encode()).hexdigest()
            entry = texts.setdefault(key, (text, [], set()))
            entry[1].append(label)
            entry[2].add(filename)

    lines = [
        "# Third-party licenses",
        "",
        "snapall is licensed under the GNU General Public License, version 3 or (at your option) any later",
        "version (see `LICENSE`). The snapall binary contains the libraries below, each under its own",
        "license, reproduced here as those licenses require.",
        "",
        f"Generated from `Cargo.lock` for `{TARGET}`: {len(packages)} libraries.",
        "",
        "## Libraries",
        "",
        "| Library | License | Authors |",
        "|---|---|---|",
    ]
    for label, license_expression, authors in rows:
        lines.append(f"| {label} | {license_expression} | {authors.replace('|', '/')} |")

    if missing:
        lines += [
            "",
            "## Libraries that ship no license file",
            "",
            "These crates declare their license in their manifest but include no separate text; the",
            "declared license and authors are the notice.",
            "",
        ]
        for label, license_expression, authors in missing:
            lines.append(f"- **{label}**: {license_expression}" + (f" ({', '.join(authors)})" if authors else ""))

    lines += ["", "## License texts", ""]
    for number, (text, crates, names) in enumerate(
        sorted(texts.values(), key=lambda entry: (entry[1][0], entry[0])), start=1
    ):
        lines += [
            f"### {number}. Used by: {', '.join(sorted(set(crates)))}",
            "",
            f"Files: {', '.join(sorted(names))}",
            "",
            "~~~~text",
            text,
            "~~~~",
            "",
        ]

    with open(OUT, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")
    print(f"wrote {os.path.normpath(OUT)}: {len(packages)} libraries, {len(texts)} distinct license texts, "
          f"{len(missing)} without a license file")


if __name__ == "__main__":
    main()
