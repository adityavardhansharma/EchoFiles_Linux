#!/usr/bin/env python3
"""Create a versioned source archive and checksummed Arch package recipe."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('version', help='Stable version matching Cargo.toml, e.g. 0.1.0')
parser.add_argument('--output', default='dist')
args = parser.parse_args()
version = args.version
if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
    parser.error('Use a stable major.minor.patch version, for example 0.1.0')
root = Path(__file__).resolve().parents[1]
manifest = subprocess.check_output(['git', 'show', 'HEAD:Cargo.toml'], cwd=root)
if tomllib.loads(manifest.decode())['workspace']['package']['version'] != version:
    parser.error('Version must match the committed workspace version in Cargo.toml')
out = Path(args.output).resolve()
out.mkdir(parents=True, exist_ok=True)
archive = out / f'echofiles-{version}-source.tar.gz'
subprocess.run(['git', 'archive', '--format=tar.gz', f'--prefix=echofiles-{version}/',
                f'--output={archive}', 'HEAD'], cwd=root, check=True)
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
recipe = (root / 'packaging/arch/PKGBUILD.in').read_text()
(out / 'PKGBUILD').write_text(recipe.replace('@VERSION@', version).replace('@SHA256@', digest))
print(f'Prepared {archive.name} with SHA-256 {digest}')
