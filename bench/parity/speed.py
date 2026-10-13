#!/usr/bin/env python3
"""fd vs ef timings: median of N warm runs per command. Usage: speed.py [--runs N] [CASE…]

Each case is a label and fd arguments; ef runs the same arguments as `ef fd`, both read
live (`--live`) and from the index (default: the index where one covers the folder).
"""

import argparse
import os
import statistics
import subprocess
import time

HERE = os.path.dirname(os.path.abspath(__file__))
EF = os.path.join(HERE, "..", "..", "target", "release", "ef")
HOME = os.path.expanduser("~")
PROJECTS = "/media/data106/Projects"

CASES = [
    ("home: name", ["report", HOME]),
    ("home: regex", ["^screenshot.*png$", HOME]),
    ("home: all files -HI", ["-HI", "-t", "f", ".", HOME]),
    ("home: ext + size", ["-e", "pdf", "-S", "+100k", ".", HOME]),
    ("home: changed today", ["--changed-within", "1d", "-t", "f", ".", HOME]),
    ("home: glob -p", ["-p", "-g", "**/Documents/**/*.md", HOME]),
    ("projects: -e rs", ["-e", "rs", ".", PROJECTS]),
    ("projects: -HI all", ["-HI", ".", PROJECTS]),
    ("projects: word", ["index", PROJECTS]),
]


def timed(cmd, runs):
    subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    times = []
    for _ in range(runs):
        t = time.perf_counter()
        p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        times.append(time.perf_counter() - t)
    return statistics.median(times) * 1000, p.stdout.count(b"\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=7)
    ap.add_argument("cases", nargs="*")
    a = ap.parse_args()
    print(f"{'case':<26}{'fd ms':>9}{'ef live':>9}{'ef index':>10}{'results':>9}  speedup")
    for label, args in CASES:
        if a.cases and not any(c in label for c in a.cases):
            continue
        fd, n_fd = timed(["fd"] + args, a.runs)
        live, n_live = timed([EF, "fd", "--live"] + args, a.runs)
        idx, n_idx = timed([EF, "fd", "--indexed"] + args, a.runs)
        same = "" if n_fd == n_live == n_idx else f"  (counts fd {n_fd} / live {n_live} / index {n_idx})"
        print(f"{label:<26}{fd:>9.1f}{live:>9.1f}{idx:>10.1f}{n_fd:>9}  {fd / min(live, idx):.1f}x{same}")


if __name__ == "__main__":
    main()
