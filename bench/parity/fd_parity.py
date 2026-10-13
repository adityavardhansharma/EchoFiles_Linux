#!/usr/bin/env python3
"""ef fd vs fd: same arguments, same results — read live and from the index.

Builds random trees (gitignore rules with negations and anchors, .ignore/.fdignore,
nested repos, hidden files, symlinks incl. broken and looping ones, fifos, empty and
executable files, node_modules and CACHEDIR.TAG folders the index skips), indexes them
with a throwaway EchoFiles config, then runs a matrix of fd options three ways:

    fd ARGS          ef fd --live ARGS          ef fd --indexed ARGS

and compares the sorted outputs. Usage: fd_parity.py [--trees N] [--cases N] [--seed S]
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

WORDS = ["src", "lib", "main", "test", "Docs", "build", "target", "a", "b", "data", "Report",
         "résumé", "notes", "x y", "café", "README", "config", "log", "tmp", "util", "deep"]
EXTS = ["rs", "txt", "md", "log", "py", "TXT", "json", "", "tar.gz", "o"]
RULES = ["*.log", "!keep.log", "target/", "/build", "tmp", "**/deep/*.txt", "*.o", "!*.md",
         "a/b", "data/**", "#comment", "notes*", "\\#x", "src/*.py", "*.TXT", "x\\ y", "café*"]


def rand_name(r):
    w = r.choice(WORDS)
    if r.random() < 0.2:
        w = "." + w
    if r.random() < 0.3:
        w += str(r.randint(0, 9))
    return w


def build_tree(root, r):
    os.makedirs(root, exist_ok=True)
    dirs = [root]
    for _ in range(r.randint(8, 30)):
        parent = r.choice(dirs)
        if parent.count(os.sep) - root.count(os.sep) > 4:
            continue
        d = os.path.join(parent, rand_name(r))
        if not os.path.exists(d):
            os.makedirs(d)
            dirs.append(d)
    for _ in range(r.randint(20, 90)):
        parent = r.choice(dirs)
        ext = r.choice(EXTS)
        name = rand_name(r) + ("." + ext if ext else "")
        p = os.path.join(parent, name)
        if os.path.exists(p):
            continue
        size = r.choice([0, 0, 10, 300, 2000, 5000])
        with open(p, "wb") as f:
            f.write(b"x" * size)
        if r.random() < 0.15:
            os.chmod(p, 0o755)
    # rule files
    for d in r.sample(dirs, min(len(dirs), r.randint(1, 5))):
        kind = r.choice([".gitignore", ".gitignore", ".ignore", ".fdignore"])
        with open(os.path.join(d, kind), "w") as f:
            f.write("\n".join(r.sample(RULES, r.randint(1, 6))) + "\n")
    # repositories
    os.makedirs(os.path.join(root, ".git"), exist_ok=True)
    if r.random() < 0.5:
        sub = r.choice(dirs)
        os.makedirs(os.path.join(sub, ".git"), exist_ok=True)
    # skipped-by-index folders with contents
    for name in ["node_modules", "cachey"]:
        d = os.path.join(r.choice(dirs), name)
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "inside.js"), "w") as f:
            f.write("y")
        if name == "cachey":
            with open(os.path.join(d, "CACHEDIR.TAG"), "w") as f:
                f.write("Signature: 8a477f597d28d172789f06886806bc55\n")
    # specials
    files = [os.path.join(dp, f) for dp, _, fs in os.walk(root) for f in fs]
    if files:
        os.symlink(r.choice(files), os.path.join(r.choice(dirs), "lnk_file"))
    os.symlink(r.choice(dirs), os.path.join(r.choice(dirs), "lnk_dir"))
    os.symlink("/nonexistent/x", os.path.join(r.choice(dirs), "broken"))
    os.symlink("..", os.path.join(r.choice(dirs), "loop"))
    os.mkfifo(os.path.join(r.choice(dirs), "fifo"))
    os.makedirs(os.path.join(r.choice(dirs), "emptydir"), exist_ok=True)


FLAGS = [[], ["-H"], ["-I"], ["-u"], ["--no-ignore-vcs"], ["--no-require-git"], ["-t", "f"],
         ["-t", "d"], ["-t", "l"], ["-t", "x"], ["-t", "e"], ["-t", "p"], ["-e", "txt"],
         ["-e", "rs", "-e", "md"], ["-S", "+1k"], ["-S", "-100b"], ["-d", "2"], ["--min-depth", "2"],
         ["--exact-depth", "1"], ["-E", "src"], ["-E", "*.md"], ["--prune"], ["-L"], ["-a"],
         ["-s"], ["-i"], ["--ignore-contain", "README"], ["--no-ignore-parent"], ["-0"],
         ["--format", "{/}|{//}|{.}"], ["--color=always"], ["--color=always", "-L"], ["-l"],
         ["-o", os.environ.get("USER", "root")], ["-o", "!root"], ["--changed-within", "1h"],
         ["--changed-before", "1h"], ["--one-file-system"], ["-j", "1"], ["--hyperlink=always"],
         ["--path-separator", "\\"], ["-t", "f", "-x", "echo", "{/.}"]]
PATTERNS = [[], ["a"], ["^a"], [r"\.rs$"], ["A"], ["-g", "*.txt"], ["-g", "**/src/**", "-p"],
            ["-F", "."], ["-p", "deep"], ["main", "--and", "rs"], ["e"], ["résumé"], ["x y"],
            ["--exact", "README"], ["--exact", "readme"], ["-p", "--exact", "x"], ["-F", "a.", "--regex"],
            ["-g", "*.txt", "--regex", "txt"], ["--regex", "-g", "*.TXT"], ["a", "--and", "R"],
            ["[A-Z]"], [r"\S+\.rs$"], ["-q", "a"], ["-a", "-l", "x"], ["-g", "-F", "x"]]


def run(cmd, cwd, env):
    p = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True)
    out = p.stdout
    nul = "-0" in cmd and "-X" not in cmd
    lines = sorted(out.split(b"\0" if nul else b"\n"))
    return [l for l in lines if l], p.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trees", type=int, default=6)
    ap.add_argument("--cases", type=int, default=150)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--keep", action="store_true")
    ap.add_argument("--served", action="store_true", help="answer through a running ef server (as the app does)")
    a = ap.parse_args()
    base = tempfile.mkdtemp(prefix="ef-parity-")
    fails = total = 0
    try:
        for t in range(a.trees):
            r = random.Random(a.seed * 1000 + t)
            tree = os.path.join(base, f"tree{t}")
            build_tree(tree, r)
            env = dict(os.environ)
            env["XDG_CONFIG_HOME"] = os.path.join(base, f"cfg{t}")
            env["XDG_CACHE_HOME"] = os.path.join(base, f"cache{t}")
            env["XDG_STATE_HOME"] = os.path.join(base, f"state{t}")
            env["XDG_RUNTIME_DIR"] = os.path.join(base, f"run{t}")
            env["HOME"] = os.path.join(base, f"home{t}")  # no global git/fd ignore files
            # Alternate between fd's own palette and a custom LS_COLORS.
            if t % 2:
                env["LS_COLORS"] = "di=01;34:ln=36:or=31:ex=01;32:*.rs=4;38;5;48:*.RS=1:*.txt=0;33:pi=40;33"
            else:
                env.pop("LS_COLORS", None)
            for k in ["XDG_CONFIG_HOME", "XDG_RUNTIME_DIR", "HOME"]:
                os.makedirs(env[k], exist_ok=True)
            os.makedirs(os.path.join(env["XDG_CONFIG_HOME"], "echofiles"), exist_ok=True)
            with open(os.path.join(env["XDG_CONFIG_HOME"], "echofiles", "settings.toml"), "w") as f:
                f.write(f'[search]\nroots = ["{tree}"]\nexclude_names = ["node_modules", ".git"]\nindex_drives = false\n')
            subprocess.run([EF, "index"], env=env, check=True, capture_output=True)
            server = None
            if a.served:
                server = subprocess.Popen([os.path.join(os.path.dirname(EF), "examples", "ef_server")], env=env, stderr=subprocess.PIPE)
                server.stderr.readline()
            for c in range(a.cases // a.trees):
                args = r.choice(PATTERNS) + r.choice(FLAGS) + (r.choice(FLAGS) if r.random() < 0.4 else [])
                if "-x" in args and args.index("-x") != len(args) - 3:
                    continue
                where = r.choice([[], ["."], [os.path.relpath(r.choice([d for d, _, _ in os.walk(tree)]), tree)]])
                # `-x … ;` must stay last.
                if "-x" in args:
                    full = args[:-3] + where + args[-3:]
                else:
                    full = args + where
                if (where and where[0] not in (".",) and not os.path.isdir(os.path.join(tree, where[0]))):
                    continue
                want, wcode = run(["fd", "--color=never"] + full, tree, env)
                for mode in ["--live", "--indexed"]:
                    total += 1
                    got, gcode = run([EF, "fd", mode, "--color=never"] + full, tree, env)
                    if wcode == 2 and gcode == 2:
                        continue
                    if got != want or (wcode != gcode):
                        fails += 1
                        print(f"\nDIFF tree{t} ({mode}): fd {' '.join(full)}  [exit {wcode} vs {gcode}]")
                        ws, gs = set(want), set(got)
                        for l in sorted(ws - gs)[:8]:
                            print("  only fd:", l.decode(errors="replace"))
                        for l in sorted(gs - ws)[:8]:
                            print("  only ef:", l.decode(errors="replace"))
            if server:
                server.kill()
        print(f"\n{total - fails}/{total} identical")
        if fails:
            print(f"trees kept in {base}")
            a.keep = True
    finally:
        if not a.keep:
            shutil.rmtree(base, ignore_errors=True)
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
