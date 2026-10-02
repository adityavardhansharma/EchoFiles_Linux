#!/usr/bin/env python3
"""Run Flea's three GUI field workloads against EchoFiles and Flea on one machine.

Warm-cache mode is the default. Cold page-cache mode requires passwordless sudo for
`sync; echo 3 > /proc/sys/vm/drop_caches` and is selected with --cold.
The fixture format follows flea tools/flea-bench, flea-media-fixture, and
flea-matched-fixture. This harness never removes a fixture without its marker.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
BASE = Path.home() / '.cache/echofiles-bench/flea-field'
MARKERS = {'scale': '.flea-bench-fixture', 'media': '.flea-media-fixture',
           'matched': '.flea-matched-fixture'}
COUNTS = {'scale': 100000, 'media': 2000, 'matched': 1700}


def command(*args, **kw):
    return subprocess.run(args, check=True, **kw)


def payload(path):
    return sum(not p.name.startswith('.') for p in path.iterdir())


def prepare(path, kind):
    marker = path / MARKERS[kind]
    if path.exists():
        if not marker.exists():
            raise RuntimeError(f'{path} exists without {marker.name}; refusing to replace it')
        if payload(path) == COUNTS[kind]:
            return False
        shutil.rmtree(path)
    path.mkdir(parents=True)
    marker.touch()
    return True


def build_scale(path):
    if prepare(path, 'scale'):
        for i in range(100000):
            (path / f'some_file_name_{i}.txt').touch()


def build_media(path):
    if not prepare(path, 'media'):
        return
    for tool in ('ffmpeg', 'magick', 'heif-enc'):
        if not shutil.which(tool):
            raise RuntimeError(f'{tool} is needed to build Flea media fixture')
    seed = {e: path / f'.seed.{e}' for e in ('png', 'jpg', 'webp', 'heic', 'mp4', 'mkv', 'webm')}
    command('magick', '-seed', '7', '-size', '1920x1080', 'plasma:fractal',
            '-depth', '8', '-strip', str(seed['png']), stdout=subprocess.DEVNULL)
    command('magick', str(seed['png']), '-strip', '-quality', '88', str(seed['jpg']), stdout=subprocess.DEVNULL)
    command('magick', str(seed['png']), '-strip', str(seed['webp']), stdout=subprocess.DEVNULL)
    command('heif-enc', '-q', '80', str(seed['png']), '-o', str(seed['heic']), stdout=subprocess.DEVNULL)
    command('ffmpeg', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i',
            'testsrc2=size=1920x1080:rate=30', '-t', '10', '-c:v', 'libx264',
            '-preset', 'veryfast', '-pix_fmt', 'yuv420p', '-fflags', '+bitexact', str(seed['mp4']))
    command('ffmpeg', '-loglevel', 'error', '-y', '-i', str(seed['mp4']), '-c',
            'copy', '-fflags', '+bitexact', str(seed['mkv']))
    command('ffmpeg', '-loglevel', 'error', '-y', '-i', str(seed['mp4']), '-c:v',
            'libvpx-vp9', '-deadline', 'realtime', '-cpu-used', '8', '-threads', '1',
            '-b:v', '2M', '-fflags', '+bitexact', str(seed['webm']))
    for i in range(2000):
        slot = i % 10
        ext, name = (('jpg', 'photo') if slot < 3 else ('png', 'image') if slot == 3
                     else ('webp', 'image') if slot == 4 else ('heic', 'image') if slot == 5
                     else ('mp4', 'clip') if slot < 8 else
                     (('mkv' if i % 20 < 10 else 'webm'), 'clip') if slot == 8
                     else ('txt', 'notes'))
        target = path / f'{name}_{i}.{ext}'
        if ext == 'txt':
            target.write_text('the long tail, so icon resolution is exercised too\n')
        else:
            shutil.copyfile(seed[ext], target)
    for p in seed.values():
        p.unlink()


def build_matched(path, media):
    if prepare(path, 'matched'):
        for p in media.iterdir():
            if p.suffix not in ('.heic', '.mkv') and not p.name.startswith('.'):
                shutil.copyfile(p, path / p.name)


def clients():
    try:
        return json.loads(subprocess.check_output(['hyprctl', '-j', 'clients'], stderr=subprocess.DEVNULL))
    except (subprocess.CalledProcessError, json.JSONDecodeError):
        return []


def descendants(pid):
    found = {pid}
    rows = subprocess.check_output(['ps', '-e', '-o', 'pid=,ppid='], text=True)
    pairs = [tuple(map(int, row.split())) for row in rows.splitlines()]
    while True:
        more = {p for p, pp in pairs if pp in found} - found
        if not more:
            return found
        found |= more


def ticks(pid):
    try:
        fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
        return sum(int(fields[i]) for i in (11, 12, 13, 14))
    except (OSError, ValueError, IndexError):
        return 0


def pss(pid):
    try:
        for line in Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines():
            if line.startswith('Pss:'):
                return int(line.split()[1])
    except OSError:
        pass
    return 0


def thumbnails(cache):
    root = cache / 'thumbnails'
    return sum(1 for p in root.glob('*/*') if p.is_file() and p.parent.name != 'fail')


def capability(app, binary, media):
    folder = BASE / 'capability'
    folder.mkdir(exist_ok=True)
    files = {}
    for ext in ('jpg', 'png', 'webp', 'heic', 'mp4', 'webm', 'mkv', 'txt'):
        source = next(media.glob(f'*.{ext}'))
        target = folder / f'sample.{ext}'
        shutil.copyfile(source, target)
        files[ext] = target
    cache = BASE / f'cache-capability-{app}'
    one(app, binary, folder, cache, False, 45, hold=45)
    found = {p.stem for p in (cache / 'thumbnails').glob('*/*') if p.is_file() and p.parent.name != 'fail'}
    return {ext: hashlib.md5(p.as_uri().encode()).hexdigest() in found for ext, p in files.items()}


def one(app, binary, fixture, cache, cold, timeout, hold=0):
    if cold:
        command('sudo', '-n', 'sh', '-c', 'sync; echo 3 > /proc/sys/vm/drop_caches')
    # XDG_CACHE_HOME isolates thumbnail output without moving the user's cache.
    shutil.rmtree(cache, ignore_errors=True)
    cache.mkdir(parents=True)
    env = os.environ.copy()
    env['XDG_CACHE_HOME'] = str(cache)
    if app == 'flea':
        env['FLEA_UI'] = str(binary.parent.parent.parent / 'ui')
        env['FLEA_BIN'] = str(binary)
        argv = [str(binary), '--gui', str(fixture)]
    else:
        argv = [str(binary), str(fixture)]
    known = {c['address'] for c in clients()}
    start = time.monotonic()
    proc = subprocess.Popen(argv, env=env, start_new_session=True,
                            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL)
    mapped = None
    settled = None
    peak_pss = 0
    cpu_start = sum(ticks(p) for p in descendants(proc.pid))
    last_cpu = None
    idle_since = None
    try:
        while time.monotonic() - start < timeout:
            now = time.monotonic()
            if mapped is None:
                for c in clients():
                    if c['address'] not in known and c['pid'] == proc.pid and c['size'][0] > 0:
                        mapped = (now - start) * 1000
                        break
            pids = descendants(proc.pid)
            cpu = sum(ticks(p) for p in pids)
            peak_pss = max(peak_pss, pss(proc.pid))
            if mapped is not None:
                if cpu == last_cpu:
                    idle_since = idle_since or now
                    if now - idle_since >= 0.5 and now - start >= hold:
                        settled = (now - start) * 1000
                        break
                else:
                    idle_since = None
            last_cpu = cpu
            if proc.poll() is not None:
                break
            time.sleep(0.02)
        return {'mapped_ms': mapped, 'settled_ms': settled,
                'pss_mib': round(peak_pss / 1024, 2),
                'cpu_s': round((cpu - cpu_start) / os.sysconf('SC_CLK_TCK'), 3),
                'thumbs': thumbnails(cache)}
    finally:
        try:
            os.killpg(proc.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
        time.sleep(0.5)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--flea', type=Path, default=Path('/tmp/flea-benchmark/target/release/flea'))
    ap.add_argument('--echofiles', type=Path, default=ROOT / 'target/release/echofiles')
    ap.add_argument('--runs', type=int, default=3)
    ap.add_argument('--cold', action='store_true')
    ap.add_argument('--timeout', type=int, default=120)
    ap.add_argument('--out', type=Path, default=ROOT / 'bench/flea-field-results.csv')
    args = ap.parse_args()
    if args.runs < 1:
        ap.error('--runs must be positive')
    args.out.parent.mkdir(parents=True, exist_ok=True)
    for path in (args.flea, args.echofiles):
        if not path.is_file():
            ap.error(f'missing binary: {path}')
    if args.cold and subprocess.run(['sudo', '-n', 'true'], capture_output=True).returncode:
        if not os.isatty(0):
            ap.error('--cold needs a terminal for sudo authentication')
        command('sudo', '-v')
    if any(c['class'].lower() in ('echofiles', 'com.thisisgm.flea') for c in clients()):
        ap.error('close existing EchoFiles and Flea windows before benchmarking')
    BASE.mkdir(parents=True, exist_ok=True)
    if subprocess.check_output(['findmnt', '-no', 'FSTYPE', '-T', str(BASE)], text=True).strip() != 'btrfs':
        ap.error('fixtures must live on btrfs, as in Flea’s field benchmark')
    fixtures = {k: BASE / k for k in COUNTS}
    build_scale(fixtures['scale'])
    build_media(fixtures['media'])
    build_matched(fixtures['matched'], fixtures['media'])
    for kind, folder in fixtures.items():
        if payload(folder) != COUNTS[kind]:
            raise RuntimeError(f'{kind} has {payload(folder)} payloads, expected {COUNTS[kind]}')
    rows = []
    for kind, folder in fixtures.items():
        for app, binary in (('flea', args.flea), ('echofiles', args.echofiles)):
            for run in range(1, args.runs + 1):
                result = one(app, binary, folder, BASE / f'cache-{app}', args.cold, args.timeout)
                row = {'fixture': kind, 'app': app, 'run': run, 'cache_mode': 'cold' if args.cold else 'warm', **result}
                rows.append(row)
                print(row, flush=True)
                with args.out.open('w', newline='') as f:
                    w = csv.DictWriter(f, rows[0].keys())
                    w.writeheader()
                    w.writerows(rows)
    for kind in fixtures:
        for app in ('flea', 'echofiles'):
            samples = [r for r in rows if r['fixture'] == kind and r['app'] == app]
            print(kind, app, {k: round(statistics.median(r[k] for r in samples), 2)
                              for k in ('mapped_ms', 'settled_ms', 'pss_mib', 'cpu_s', 'thumbs')
                              if all(r[k] is not None for r in samples)})
    capabilities = {app: capability(app, binary, fixtures['media'])
                    for app, binary in (('flea', args.flea), ('echofiles', args.echofiles))}
    cap_path = args.out.with_name('flea-capability-results.json')
    cap_path.write_text(json.dumps(capabilities, indent=2) + '\n')
    print('capability', capabilities)


if __name__ == '__main__':
    main()
