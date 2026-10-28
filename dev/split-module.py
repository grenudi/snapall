#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Split one big module file into a directory of modules by purpose, moving whole items and rewriting none.

    dev/split-module.py SPEC.json

SPEC.json:
    {"dir": "/abs/path/to/the/folder/holding/the/file/",       the folder that holds NAME.rs
     "name": "health",                                         NAME.rs becomes NAME/mod.rs plus one file per group
     "tests_purpose": "One sentence for the moved tests.",     the `mod tests { .. }` moves to NAME/tests.rs
     "groups": {"facts": ["What this module is for.", ["Item", "impl Item", "a_function"]], ...}}

Every top-level item must be placed in exactly one group, by name; an `impl` block is named by its head
(`impl Item`, `impl Trait for Item`). A group's public names are re-exported by the new front. Afterwards run
`cargo fmt`, `cargo fix --lib --tests --allow-dirty` and `cargo check`; what the compiler reports is only visibility
(`pub(super)` -> `pub(crate)`) and imports. Refresh the outline: UPDATE_OUTLINE=1 cargo test -p snapall --test architecture,
and pay the debts that were paid: UPDATE_RATCHETS=1 on the same test.
"""
import re, os, sys, json
spec=json.load(open(sys.argv[1]))
D=spec["dir"]; NAME=spec["name"]
HDR="// SPDX-FileCopyrightText: 2026 grenudi\n//\n// SPDX-License-Identifier: GPL-3.0-or-later\n\n"
whole=open(f"{D}{NAME}.rs").read()
tm=re.search(r"#\[cfg\(test\)\]\nmod tests \{",whole)
if tm:
    end=whole.index("\n}\n",tm.start())+3          # the first closing brace in column 0 ends `mod tests`
    code=(whole[:tm.start()]+whole[end:]).split("\n"); tests=whole[tm.start():end].rstrip("\n")+"\n"
else:
    code=whole.split("\n"); tests=""
doc=[l for l in code if l.startswith("//!")]
first=next(i for i,l in enumerate(code) if l.startswith("use "))
chunks=[]; i=first
while i<len(code):
    if code[i].startswith("use "):
        j=i
        while not code[j].rstrip().endswith(";"): j+=1
        chunks.append("\n".join(code[i:j+1])); i=j+1
    elif not code[i].strip() or code[i].startswith("//"): i+=1
    else: break
imports="\n".join(chunks); body_start=i
def items():
    out=[]; i=body_start
    while i<len(code):
        if not code[i].strip() or (code[i].startswith("//") and not code[i].startswith("///")): i+=1; continue
        start=i; j=i
        while j<len(code) and (code[j].startswith("///") or code[j].startswith("#[")): j+=1
        head=code[j]; end=j
        single=(head.rstrip().endswith(";") and "{" not in head) or head.rstrip().endswith("{}")
        if not single:
            while end<len(code) and code[end] not in ("}","};"): end+=1
        out.append((start,end,head)); i=end+1
    return out
def name_of(h):
    m=re.match(r"(?:pub(?:\([a-z]+\))? )?(?:fn|struct|enum|const|type|static|trait) (\w+)",h)
    return m.group(1) if m else re.sub(r"\s*\{.*$","",h).rstrip()
groups=spec["groups"]
where={n:g for g,(_,ns) in groups.items() for n in ns}
buckets={g:[] for g in groups}
for s,e,h in items():
    n=name_of(h)
    if n not in where: raise SystemExit("unplaced: "+n+" :: "+h[:70])
    buckets[where[n]].append(("\n".join(code[s:e+1]),n,h.startswith("pub "),h.startswith("pub(crate) ")))
os.makedirs(f"{D}{NAME}",exist_ok=True)
def widen(b): return re.sub(r"^(fn|struct|enum|const|static|type|trait) ",lambda m:"pub(super) "+m.group(1)+" ",b,flags=re.M)
pub_by={}; crate_by={}; priv_by={}
for g,bl in buckets.items():
    text="\n\n".join(b if (p or c) else widen(b) for b,n,p,c in bl)
    open(f"{D}{NAME}/{g}.rs","w").write(HDR+f"//! {groups[g][0]}\n\nuse super::*;\n{imports}\n\n"+text+"\n")
    named=[(b,n) for b,n,p,c in bl if not n.startswith("impl")]
    pub_by[g]=[n for b,n,p,c in bl if p and not n.startswith("impl")]
    crate_by[g]=[n for b,n,p,c in bl if c and not n.startswith("impl")]
    priv_by[g]=[n for b,n,p,c in bl if not p and not c and not n.startswith("impl")]
if tests:
    inner=tests.split("mod tests {",1)[1].rstrip(); assert inner.endswith("}"); inner=inner[:-1]
    open(f"{D}{NAME}/tests.rs","w").write(HDR+f"//! {spec['tests_purpose']}\n\n#![cfg(test)]\n\n"+imports+"\n"+inner+"\n")
front=HDR+"\n".join(doc)+"\n\n"+"\n".join(f"mod {g};" for g in groups)+("\n#[cfg(test)]\nmod tests;\n" if tests else "\n")+"\n"
front+="\n".join(f"pub use {g}::{{{', '.join(sorted(ns))}}};" for g,ns in pub_by.items() if ns)+"\n"
front+="\n".join(f"pub(crate) use {g}::{{{', '.join(sorted(ns))}}};" for g,ns in crate_by.items() if ns)+"\n"
front+="\n".join(f"use {g}::{{{', '.join(sorted(ns))}}};" for g,ns in priv_by.items() if ns)+"\n"
open(f"{D}{NAME}/mod.rs","w").write(front)
os.remove(f"{D}{NAME}.rs"); print({g:len(b) for g,b in buckets.items()})
