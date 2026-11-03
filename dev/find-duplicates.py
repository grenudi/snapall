#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Find the same code written in more than one place.

    dev/find-duplicates.py [--lines N] [--min-chars C] [--top T] [--near]

A rule copied by hand is a rule that can come out differently in each copy, and it is the commonest thing a reading of the code
finds: a `matches!` written three times, a classification repeated on a type that already held it. Reading finds them one
module at a time; this finds them all at once. It looks at production code only (tests repeat on purpose), drops comments and
the lines that carry no meaning on their own (braces, attributes, imports), and reports every run of N lines (default 4) that
appears in two or more places, longest and most repeated first. With --near, names, numbers and strings are forgotten and only
the shape is compared: the same code under different names, which is how a rule gets copied and then renamed to fit. The identical
copies are left to the exact scan, which is also a test. A repeated run is a candidate, not a verdict: two commands
that begin the same way may be right to. Read each, and name the rule if there is one.
"""
import collections, glob, re, sys

TRIVIAL = re.compile(r"^(\}|\{|\)|\];?|\);?|\},?|\)\)?;?|\)\?;?|else \{|#\[.*|use .*|mod .*|pub mod .*|pub use .*|//.*|\)\.collect\(\);?)$")


KEYWORDS = set("as break const continue crate else enum fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait type unsafe use where while true false Some None Ok Err".split())


def shape(line):
    """The line with its names, numbers and strings forgotten: what is left is how it is built."""
    line = re.sub(r'"(?:\\.|[^"\\])*"', '"S"', line)
    line = re.sub(r"\b\d[\d_]*(?:\.\d+)?\b", "N", line)
    return re.sub(r"\b[A-Za-z_]\w*\b", lambda m: m.group(0) if m.group(0) in KEYWORDS else "I", line)


def production_lines(path):
    text = open(path).read()
    if "#![cfg(test)]" in text or path.endswith("tests.rs") or "/tests/" in path:
        return []
    code = text.split("#[cfg(test)]")[0]
    out = []
    for number, line in enumerate(code.split("\n"), 1):
        norm = " ".join(line.split())
        if not norm or TRIVIAL.match(norm):
            continue
        out.append((number, norm))
    return out


NEAR = "--near" in sys.argv


def main(argv):
    option = lambda name, default: int(next((argv[i + 1] for i, a in enumerate(argv) if a == f"--{name}" and i + 1 < len(argv)), default))
    window, min_chars, top = option("lines", 4), option("min-chars", 170), option("top", 25)
    seen = collections.defaultdict(list)
    for path in sorted(glob.glob("crates/*/src/**/*.rs", recursive=True)):
        lines = production_lines(path)
        for at in range(len(lines) - window + 1):
            chunk = lines[at : at + window]
            body = "\n".join(text for _, text in chunk)
            if len(body) >= min_chars:
                key = shape(body) if NEAR else body
                seen[key].append((path.replace("crates/", ""), chunk[0][0], chunk[-1][0], body))
    found = []
    for body, places in seen.items():
        if len(places) < 2:
            continue
        # near: the same shape under different names; the identical ones are the exact scan's, and a guard's
        if NEAR and len({original for *_, original in places}) < 2:
            continue
        found.append((len(places) * len(body), body, places))
    found.sort(key=lambda item: -item[0])
    reported = []
    shown = 0
    for weight, body, places in found:
        # a longer run reports its overlapping shorter ones too: show each place once
        if any(any(p[0] == q[0] and p[1] <= q[2] and q[1] <= p[2] for p in places for q in earlier) for earlier in reported):
            continue
        reported.append(places)
        shown += 1
        print(f"\n{len(places)}x  ({len(body)} chars)")
        for path, first, last, _ in places:
            print(f"    {path}:{first}-{last}")
        for line in (places[0][3] if NEAR else body).split("\n")[:window]:
            print(f"      | {line[:110]}")
        if shown >= top:
            break
    print(f"\n{len(found)} repeated runs of {window} lines; {shown} shown")


if __name__ == "__main__":
    main(sys.argv[1:])
