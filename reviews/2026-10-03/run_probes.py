#!/usr/bin/env python3
"""Reproduce the recorded bad behaviors on the reviewed revision; not a safety test suite.
Run from any directory. Uses temporary files and temporary Cargo integration-test files.
Phone connections target localhost, but Service::start uses its normal network listeners
and discovery announcement. No real phone interaction is required or intended.
"""
from pathlib import Path
import os
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent

def run(args, **kwargs):
    return subprocess.run(args, cwd=REPO, check=True, **kwargs)

def standalone(name, libraries, cross_device=False):
    with tempfile.TemporaryDirectory(prefix='ef-review-') as raw:
        scratch = Path(raw)
        command = ['rustc', '--edition=2024', '-Awarnings', str(HERE / name),
                   '-L', 'dependency=target/debug/deps', '-o', str(scratch / 'probe')]
        for lib in libraries:
            matches = list((REPO / 'target/debug/deps').glob(f'lib{lib}-*.rlib'))
            artifact = max(matches, key=lambda path: path.stat().st_mtime)
            command += ['--extern', f'{lib}={artifact}']
        run(command)
        env = os.environ.copy()
        (scratch / 'data').mkdir()
        env['XDG_DATA_HOME'] = str(scratch / 'data')
        args = [str(scratch / 'probe'), str(scratch / 'cases')]
        if cross_device:
            with tempfile.TemporaryDirectory(prefix='ef-review-', dir='/dev/shm') as other:
                run(args + [other], env=env)
        else:
            run(args, env=env)

def integration(crate, package, source, test_filter=None):
    folder = REPO / 'crates' / crate / 'tests'
    created_folder = not folder.exists()
    folder.mkdir(exist_ok=True)
    target = folder / 'deep_review_probe.rs'
    # Exclusive creation refuses to overwrite an existing test or developer work.
    with target.open('x') as stream:
        stream.write((HERE / source).read_text())
    try:
        args = ['cargo', 'test', '--locked', '-p', package, '--test', 'deep_review_probe']
        if test_filter:
            args.append(test_filter)
        run(args + ['--', '--nocapture'])
    finally:
        target.unlink()
        if created_folder:
            folder.rmdir()

if __name__ == '__main__':
    run(['cargo', 'test', '--workspace', '--locked'])
    standalone('core_index_probe.rs', ['ef_core', 'ef_index'], cross_device=True)
    standalone('search_state_probe.rs', ['ef_index', 'ef_config', 'rayon'])
    integration('phone', 'echofiles-phone', 'phone_probe.rs')
    integration('ui', 'echofiles', 'ui_probe.rs', 'audit_hidden_file_targets')
