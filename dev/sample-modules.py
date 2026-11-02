#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Pick modules at random to read as a stranger, because that finds what no guard measures.

    dev/sample-modules.py [SEED] [PER_LAYER]       defaults: today's date as the seed, 2 per layer

The proxies (sizes, the outline, the findability test) are all things someone thought to measure. Reading code with no memory of
having written it finds what nobody thought of: stale words in messages, a rule copied by hand, the same sentence written twice.
Sampling at random, with a seed so a read can be reproduced, keeps the choice from drifting to the modules already liked.
It picks modules of a size worth judging (80 to 300 lines of production code) from each layer: the core, the command line, the window and the adapters.
Read each one in full, then write down what a stranger would notice, and fix what is real.
"""
import datetime, glob, random, sys

LAYERS = {"core": "crates/snapall-core/src", "binary": "crates/snapall/src", "window": "crates/snapall-tui/src", "adapters": "crates/snapall-s*/src"}

def main(argv):
    seed = int(argv[0]) if argv else int(datetime.date.today().strftime("%Y%m%d"))
    per_layer = int(argv[1]) if len(argv) > 1 else 2
    random.seed(seed)
    print(f"seed {seed}")
    for name, pattern in LAYERS.items():
        files = []
        for path in sorted(glob.glob(pattern + "/**/*.rs", recursive=True)):
            text = open(path).read()
            if "#![cfg(test)]" in text or path.endswith(("mod.rs", "lib.rs")):
                continue
            lines = len(text.split("#[cfg(test)]")[0].split("\n"))
            if 80 <= lines <= 300:
                files.append((path, lines))
        for path, lines in random.sample(files, min(per_layer, len(files))):
            print(f"{name:9} {lines:4}  {path}")

if __name__ == "__main__":
    main(sys.argv[1:])
