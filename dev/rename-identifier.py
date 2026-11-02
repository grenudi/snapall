#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Rename an identifier across the crates without touching anything that is not the identifier.

    dev/rename-identifier.py OLD NEW [--crates a,b] [--skip path,path] [--dry]

A rename by search and replace goes wrong in ways a compiler does not see: a word inside a string is output (a JSON key,
a sentence), and a word in a comment is prose. So this renames only identifier tokens, with a lexer that sets aside
comments, ordinary strings (a trailing backslash continues one across lines) and raw strings (`r#"..."#`). The one thing it
does change inside a string is an inline `{OLD}` or `{OLD:` of a format string, which has to follow the variable it names.

It then checks what it did: every string literal on every changed line must be identical to before, apart from those
inline names. If one is not, it puts everything back and says which. (The first two attempts at a rename by regex changed a
JSON key and the text of a fake tool's output; this check is how that was found, and why it is a tool.)

A name that is already in use where it would be renamed to, or that shadows a function, is the compiler's to find: the
rename is mechanical and the collisions are for a person to name.
"""
import glob, re, subprocess, sys

# the crates that speak for the program; the adapters keep the words of the tools they drive, so they are named with --crates
CRATES = ("snapall-core", "snapall", "snapall-tui", "snapall-observe")
LEX = r'(//[^\n]*|r(#*)"(?:.|\n)*?"\2|"(?:\\.|[^"\\])*"|\b%s\b)'
STRING = re.compile(r'r(#*)"(?:.|\n)*?"\1|"(?:\\.|[^"\\])*"', re.DOTALL)


def main(argv):
    args = [a for a in argv if not a.startswith("--")]
    if len(args) != 2:
        sys.exit(__doc__)
    old, new = args
    option = lambda name: next((a.split("=", 1)[1] for a in argv if a.startswith(f"--{name}=")), "")
    crates = option("crates").split(",") if option("crates") else CRATES
    skip = tuple(s for s in option("skip").split(",") if s)
    dry = "--dry" in argv
    token = re.compile(LEX % re.escape(old), re.DOTALL)
    inline = re.compile(r"\{%s(?=[}:])" % re.escape(old))

    def replace(m):
        text = m.group(0)
        if text.startswith("//") or text.startswith("r#") or text.startswith('r"'):
            return text
        if text.startswith('"'):
            return inline.sub("{" + new, text)
        return new

    changed, uses = [], 0
    for crate in crates:
        for path in glob.glob(f"crates/{crate}/src/**/*.rs", recursive=True) + glob.glob(f"crates/{crate}/tests/**/*.rs", recursive=True):
            if path.endswith(skip):
                continue
            text = open(path).read()
            fresh = token.sub(replace, text)
            if fresh != text:
                uses += len(re.findall(r"\b%s\b" % re.escape(old), text))
                changed.append((path, text, fresh))
    print(f"{len(changed)} files would change" if dry else f"{len(changed)} files changed")
    if dry:
        return
    for path, _, fresh in changed:
        open(path, "w").write(fresh)
    diff = subprocess.run(["git", "diff", "-U0"], capture_output=True, text=True).stdout
    minus, plus, bad, file = [], [], [], None

    def flush():
        nonlocal minus, plus
        if minus or plus:
            before = [m.group(0) for line in minus for m in STRING.finditer(line)]
            before = [inline.sub("{" + new, s) for s in before]
            after = [m.group(0) for line in plus for m in STRING.finditer(line)]
            if before != after:
                bad.append((file, before, after))
        minus, plus = [], []

    for line in diff.split("\n"):
        if line.startswith("+++ b/"):
            flush(); file = line[6:]
        elif line.startswith("@@"):
            flush()
        elif line.startswith("---"):
            continue
        elif line.startswith("-"):
            minus.append(line[1:])
        elif line.startswith("+"):
            plus.append(line[1:])
    flush()
    if bad:
        for path, _, _ in changed:
            subprocess.run(["git", "checkout", "-q", "--", path])
        for file, before, after in bad[:6]:
            print(f"a string changed in {file}: {[s[:50] for s in before][:3]} -> {[s[:50] for s in after][:3]}")
        sys.exit("put everything back: a string literal would have changed")
    print("no string literal changed; the compiler finds any collision")


if __name__ == "__main__":
    main(sys.argv[1:])
