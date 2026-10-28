#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""How close the code is to "every layer is the top of the layers beneath it", measured the same way at any point in history.

    dev/structure-metrics.py                 the numbers for the working tree
    dev/structure-metrics.py --trend         one column per minor release since 0.21, then the working tree
    dev/structure-metrics.py REF [REF ...]   the numbers at each git ref (a tag, a branch, a commit)
    dev/structure-metrics.py --trend --markdown    the same as a table, for a CI summary

These are proxies. Size says little about whether a module speaks one language, so read them with the cold-read test
(crates/snapall/tests/findability.rs), which asks whether the outline alone leads a stranger to the right module. A number
that moves only because a limit was gamed (files huddled just under it) is shown on purpose: `files 250-299`.
Files marked `#![cfg(test)]` and everything after `#[cfg(test)]` are not production code and are not counted.
"""
import re, statistics, subprocess, sys

REPO_TAG = re.compile(r"^snapall-core-v0\.(\d+)\.0$")
JARGON = re.compile(r"(adapter|imperative|\bshell\b|hexagon|composition root|functional core|driving|\bport\b)", re.I)

def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True).stdout

def read(ref, name):
    if ref == "WORKTREE":
        with open(name) as handle:
            return handle.read()
    return git("show", f"{ref}:{name}")

def names(ref):
    if ref == "WORKTREE":
        out = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard"], capture_output=True, text=True).stdout
    else:
        out = git("ls-tree", "-r", "--name-only", ref)
    return [n for n in out.split("\n") if re.match(r"crates/[^/]+/src/.*\.rs$", n)]

def purpose(text):
    doc = []
    for line in text.split("\n"):
        if line.startswith("//!"):
            doc.append(line[3:].strip())
        elif doc and not line.strip():
            break
        elif doc:
            break
    return " ".join(doc)

def measure(ref):
    sources = {}
    for name in names(ref):
        text = read(ref, name)
        if not text or "#![cfg(test)]" in text:
            continue
        sources[name] = text.split("#[cfg(test)]")[0]
    lines = {n: len(t.split("\n")) for n, t in sources.items()}
    entry = [l for n, l in lines.items() if n.rsplit("/", 1)[-1] in ("lib.rs", "main.rs", "mod.rs")]
    long_functions = total_functions = 0
    for text in sources.values():
        rows = text.split("\n")
        i = 0
        while i < len(rows):
            if re.match(r"(?:pub(?:\([a-z]+\))? )?fn \w+", rows[i]):
                j = i
                while j < len(rows) and rows[j] != "}":
                    j += 1
                total_functions += 1
                long_functions += (j - i + 1) > 60
                i = j
            i += 1
    prints = sum(len(re.findall(r"\b(?:e?println|e?print)!\(", t)) for n, t in sources.items() if "snapall-tui" not in n and not n.endswith("/ask.rs"))
    # commands lived in one file before they became a folder: the same code, so the same question
    in_commands = [(n, t) for n, t in sources.items() if re.match(r"crates/snapall/src/commands(/|\.rs$)", n)]
    bare_bools = sum(len(re.findall(r"\b\w+: bool\b", row)) for _, t in in_commands for row in t.split("\n") if re.match(r"\s*(pub(\([a-z]+\))? )?fn ", row) or re.match(r"\s+\w+: bool,", row))
    purposes = {n: purpose(t) for n, t in sources.items()}
    window_state = 0
    for n, t in sources.items():
        m = re.search(r"pub struct State \{(.*?)\n\}", t, re.S) if n.startswith("crates/snapall-tui/") else None
        if m:
            window_state = len(re.findall(r"^\s+pub \w+:", m.group(1), re.M))
    biggest = max(lines.items(), key=lambda kv: kv[1])
    return {
        "source files": len(lines),
        "median file (lines)": int(statistics.median(lines.values())),
        "files over 300": sum(l > 300 for l in lines.values()),
        "files 250-299 (huddled under the limit)": sum(250 <= l <= 299 for l in lines.values()),
        "biggest file": f"{biggest[1]}",
        "entry files over 120": sum(l > 120 for l in entry),
        "core: top-level entries": len({n.split("/")[3] for n in lines if n.startswith("crates/snapall-core/src/")}),
        "functions over 60 lines": long_functions,
        "prints outside the presenter": prints,
        "bare bool params in commands": bare_bools,
        "window State fields": window_state,
        "modules with no purpose sentence": sum(1 for p in purposes.values() if not p),
        "purposes in pattern jargon": sum(1 for p in purposes.values() if JARGON.search(p.split(". ")[0])),
    }

def trend_refs():
    tags = []
    for tag in git("tag", "--list").split():
        m = REPO_TAG.match(tag)
        if m and int(m.group(1)) >= 21:
            tags.append((int(m.group(1)), tag))
    return [t for _, t in sorted(tags)] + ["WORKTREE"]

def main(argv):
    markdown = "--markdown" in argv
    args = [a for a in argv if not a.startswith("--")]
    refs = trend_refs() if "--trend" in argv else (args or ["WORKTREE"])
    results = [(r.replace("snapall-core-v", "") if r != "WORKTREE" else "now", measure(r)) for r in refs]
    keys = list(results[0][1])
    if markdown:
        print("| | " + " | ".join(label for label, _ in results) + " |")
        print("|---|" + "---|" * len(results))
        for k in keys:
            print(f"| {k} | " + " | ".join(str(m[k]) for _, m in results) + " |")
    else:
        width = max(len(k) for k in keys) + 2
        print(" " * width + "".join(f"{label:>10}" for label, _ in results))
        for k in keys:
            print(f"{k:<{width}}" + "".join(f"{str(m[k]):>10}" for _, m in results))

if __name__ == "__main__":
    main(sys.argv[1:])
