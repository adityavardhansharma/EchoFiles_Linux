#!/usr/bin/env python3
"""Time `find` and `fd` on the index benchmark corpora, for comparison with the in-process
index numbers from `cargo bench -p echofiles-index` (run that first: it generates the
corpora under ~/.cache/echofiles-bench/index/).

Usage: python3 bench/index_compare.py [runs]   (default 7; median, after one warm-up)
"""

import os
import shutil
import statistics
import subprocess
import sys
import time

RUNS = int(sys.argv[1]) if len(sys.argv) > 1 else 7
BASE = os.path.expanduser("~/.cache/echofiles-bench/index")


def median_ms(cmd):
    subprocess.run(cmd, stdout=subprocess.DEVNULL, check=True)  # warm-up
    times = []
    for _ in range(RUNS):
        t = time.perf_counter()
        out = subprocess.run(cmd, stdout=subprocess.PIPE, check=True).stdout
        times.append((time.perf_counter() - t) * 1000)
    return statistics.median(times), out.count(b"\0")  # names may contain newlines


def main():
    fd = shutil.which("fd") or shutil.which("fdfind")
    print(f"| Corpus | `find -iname '*report*'` | `fd -HI -i -F report` |")
    print("|---|---|---|")
    for layout in ("flat", "tree"):
        for n in (100, 1000, 10000, 100000):
            root = os.path.join(BASE, f"{layout}-{n}")
            if not os.path.isdir(root):
                continue
            f_ms, f_hits = median_ms(["find", root, "-iname", "*report*", "-print0"])
            cell = "not installed"
            if fd:
                d_ms, d_hits = median_ms([fd, "-H", "-I", "-i", "-F", "-0", "report", root])
                cell = f"{d_ms:.1f} ms ({d_hits} hits)"
            print(f"| {layout} {n:,} | {f_ms:.1f} ms ({f_hits} hits) | {cell} |")


if __name__ == "__main__":
    main()
