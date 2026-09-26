#!/usr/bin/env python3
"""EchoFiles vs the system file manager, measured the same way from outside both apps.

For each app and folder: start a fresh process, poll Hyprland until its window maps, then
screenshot only that window until three consecutive captures are identical (content stable:
listing done, spinners gone). Reports time to window, time to stable content, RSS and CPU time.

Usage: python3 bench/compare.py [runs]
Needs Hyprland (hyprctl) and grim. Opens and closes real windows.
"""
import hashlib, json, os, signal, subprocess, sys, time

HOME = os.path.expanduser("~")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUNS = int(sys.argv[1]) if len(sys.argv) > 1 else 3
FOLDERS = [
    ("~/Projects (small)", os.path.join(HOME, "Projects")),
    ("10k files", os.path.join(HOME, ".cache/echofiles-bench/d10k")),
    ("100k files", os.path.join(HOME, ".cache/echofiles-bench/d100k")),
]
APPS = {
    "EchoFiles": lambda d: [os.path.join(ROOT, "target/release/echofiles"), d],
    "Nautilus": lambda d: ["nautilus", "--new-window", d],
}
TIMEOUT = 90.0


def clients():
    out = subprocess.run(["hyprctl", "clients", "-j"], capture_output=True, text=True).stdout
    try:
        return json.loads(out)
    except json.JSONDecodeError:
        return []


def find_window(pids, known):
    for c in clients():
        if c["address"] in known:
            continue
        if c["pid"] in pids or c["class"].lower() in ("org.gnome.nautilus", "echofiles"):
            return c
    return None


def descendants(pid):
    pids = {pid}
    try:
        out = subprocess.run(["ps", "-e", "-o", "pid=,ppid="], capture_output=True, text=True).stdout
    except OSError:
        return pids
    rows = [tuple(map(int, l.split())) for l in out.splitlines() if l.strip()]
    changed = True
    while changed:
        changed = False
        for p, pp in rows:
            if pp in pids and p not in pids:
                pids.add(p)
                changed = True
    return pids


def rss_and_cpu(pids):
    rss_kb, ticks = 0, 0
    for p in pids:
        try:
            for line in open(f"/proc/{p}/status"):
                if line.startswith("VmRSS:"):
                    rss_kb += int(line.split()[1])
            f = open(f"/proc/{p}/stat").read().rsplit(")", 1)[1].split()
            ticks += int(f[11]) + int(f[12])
        except (OSError, IndexError, ValueError):
            pass
    return rss_kb / 1024, ticks * 1000 / os.sysconf("SC_CLK_TCK")


def shot(geom):
    r = subprocess.run(["grim", "-g", geom, "-t", "ppm", "-"], capture_output=True)
    return hashlib.blake2b(r.stdout, digest_size=16).digest()


def stop(app, proc):
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    if app == "Nautilus":
        subprocess.run(["nautilus", "-q"], capture_output=True)
        subprocess.run(["pkill", "-x", "nautilus"], capture_output=True)
    time.sleep(0.8)


def run_once(app, folder):
    known = {c["address"] for c in clients()}
    t0 = time.perf_counter()
    proc = subprocess.Popen(APPS[app](folder), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    win = None
    while time.perf_counter() - t0 < TIMEOUT:
        win = find_window(descendants(proc.pid), known)
        if win and win["size"][0] > 0:
            break
        time.sleep(0.005)
    if not win:
        stop(app, proc)
        return None
    t_window = time.perf_counter() - t0
    time.sleep(0.03)  # let the compositor settle the tile geometry
    win = next((c for c in clients() if c["address"] == win["address"]), win)
    x, y = win["at"]
    w, h = win["size"]
    geom = f"{x},{y} {w}x{h}"
    last, same, t_first_same = None, 0, None
    while time.perf_counter() - t0 < TIMEOUT:
        t = time.perf_counter() - t0
        hsh = shot(geom)
        if hsh == last:
            same += 1
            if same >= 2:
                break
        else:
            last, same, t_first_same = hsh, 0, t
    t_stable = t_first_same
    rss, cpu = rss_and_cpu(descendants(proc.pid))
    stop(app, proc)
    return t_window * 1000, t_stable * 1000, rss, cpu


def main():
    subprocess.run(["nautilus", "-q"], capture_output=True)
    results = {}
    for label, folder in FOLDERS:
        for app in APPS:
            run_once(app, folder)  # warm-up: page cache and shader caches, not counted
            samples = [r for r in (run_once(app, folder) for _ in range(RUNS)) if r]
            results[(label, app)] = samples
            if samples:
                med = lambda i: sorted(s[i] for s in samples)[len(samples) // 2]
                print(f"{label:<20} {app:<10} window {med(0):7.0f} ms · content stable {med(1):7.0f} ms · RSS {med(2):6.0f} MB · CPU {med(3):6.0f} ms   ({len(samples)} runs)", flush=True)
            else:
                print(f"{label:<20} {app:<10} no window within {TIMEOUT:.0f} s", flush=True)
    json.dump({f"{k[0]}|{k[1]}": v for k, v in results.items()}, open(os.path.join(ROOT, "bench/results.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
