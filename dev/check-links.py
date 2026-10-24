# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Fail if a relative link in any Markdown file points at a file that does not exist.

Documentation rots when files move; this is the cheap check that catches it.
    python3 dev/check-links.py [ROOT]
"""
import os, re, sys

root=sys.argv[1] if len(sys.argv) > 1 else '.'
bad=[]; total=0
for dp,dn,fn in os.walk(root):
    dn[:]=[d for d in dn if d not in ('target','.git','node_modules','brainstorms','generated')]
    for f in fn:
        if not f.endswith('.md') or f=='CHANGELOG.md': continue
        path=os.path.join(dp,f); text=open(path,errors='ignore').read()
        # skip fenced code blocks
        text=re.sub(r"```.*?```","",text,flags=re.S)
        for m in re.finditer(r"\]\(([^)\s]+)\)", text):
            target=m.group(1)
            if re.match(r"(https?:|mailto:|#)", target): continue
            total+=1
            rel=target.split('#')[0]
            if not rel: continue
            if not os.path.exists(os.path.normpath(os.path.join(dp,rel))): bad.append((os.path.relpath(path,root), target))
print(f"{root}: {total} relative links checked, {len(bad)} broken")
for p,t in bad: print("   ", p, "->", t)
sys.exit(1 if bad else 0)
