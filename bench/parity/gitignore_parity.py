#!/usr/bin/env python3
"""EchoFiles' gitignore rules vs git's own: random repositories with nested .gitignore
files full of tricky rules (negation, anchoring, `**`, folder-only, escapes, character
classes); the files `ef fd -H` lists must be exactly those `git ls-files --others
--exclude-standard` reports. Usage: gitignore_parity.py [--repos N] [--seed S]
"""

import argparse
import os
import random
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
EF = os.path.join(HERE, "..", "..", "target", "release", "ef")

NAMES = ["a", "b", "build", "lib", "src", "x.log", "y.log", "keep.log", "foo", "Foo", "foo.txt",
         "bar.o", "deep", "doc", "z[1]", "sp ace", "#hash", "!bang", "café", "a.b.c", ".hid", "t1", "t2"]
RULES = ["*.log", "!keep.log", "build/", "/foo", "foo/", "doc/*.txt", "**/deep", "deep/**", "a/**/b",
         "*.o", "!bar.o", "\\#hash", "\\!bang", "[ab]", "t[0-9]", "*", "!*/", "!src", "src/*", "café",
         "sp\\ ace", "lib", "!lib/", "**/x.log", "a.*", "/b/", "?", "**", "z\\[1\\]", "!Foo", "!/foo"]


def build(root, r):
    dirs = [root]
    for _ in range(r.randint(5, 18)):
        d = os.path.join(r.choice(dirs), r.choice(NAMES))
        if not os.path.exists(d):
            os.makedirs(d)
            dirs.append(d)
    for _ in range(r.randint(15, 60)):
        p = os.path.join(r.choice(dirs), r.choice(NAMES))
        if not os.path.exists(p):
            open(p, "w").close()
    for d in r.sample(dirs, min(len(dirs), r.randint(1, 4))):
        with open(os.path.join(d, ".gitignore"), "w") as f:
            f.write("\n".join(r.sample(RULES, r.randint(1, 6))) + "\n")
    subprocess.run(["git", "init", "-q", root], check=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repos", type=int, default=200)
    ap.add_argument("--seed", type=int, default=1)
    a = ap.parse_args()
    base = tempfile.mkdtemp(prefix="ef-gi-")
    env = dict(os.environ, HOME=base, XDG_CONFIG_HOME=os.path.join(base, "cfg"), XDG_RUNTIME_DIR=base, GIT_CONFIG_NOSYSTEM="1")
    fails = 0
    for i in range(a.repos):
        r = random.Random(a.seed * 100_000 + i)
        root = os.path.join(base, f"r{i}")
        build(root, r)
        git = subprocess.run(["git", "ls-files", "--others", "--exclude-standard", "-z"], cwd=root, env=env, capture_output=True).stdout
        want = sorted(p for p in git.split(b"\0") if p and not p.endswith(b".gitignore"))
        got = subprocess.run([EF, "fd", "-H", "-t", "f", "-E", ".git", "-0", "--strip-cwd-prefix"], cwd=root, env=env, capture_output=True).stdout
        have = sorted(p for p in got.split(b"\0") if p and not p.endswith(b".gitignore"))
        if want != have:
            fails += 1
            print(f"\nDIFF r{i}:")
            for d in sorted({d for d, _, fs in os.walk(root) if ".gitignore" in fs}):
                print(f"  {os.path.relpath(d, root)}/.gitignore: {open(os.path.join(d, '.gitignore')).read().split()}")
            for p in sorted(set(want) - set(have))[:6]:
                print("  only git:", p.decode())
            for p in sorted(set(have) - set(want))[:6]:
                print("  only ef: ", p.decode())
    print(f"\n{a.repos - fails}/{a.repos} repositories identical")
    if not fails:
        shutil.rmtree(base, ignore_errors=True)
    else:
        print(f"kept in {base}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
