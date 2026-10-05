#!/usr/bin/env python3
"""Build and audit the frozen source; all fixture writes use TemporaryDirectory.
Use ECHOFILES_REVIEW_TARGET to choose Cargo output (default: repo/target).
Cargo JSON artifacts select the snapshot libraries explicitly, never by mtime.
"""
from pathlib import Path
import json, os, subprocess, tempfile
HERE=Path(__file__).resolve().parent
REPO=HERE.parents[2]
TARGET=Path(os.environ.get('ECHOFILES_REVIEW_TARGET', str(REPO/'target'))).resolve()
env=os.environ.copy();env['CARGO_TARGET_DIR']=str(TARGET)
command=['cargo','build','--manifest-path',str(HERE/'snapshot/Cargo.toml'),'--locked','--lib','-p','echofiles-core','-p','echofiles-index','-p','echofiles-config','--message-format=json']
built=subprocess.run(command,cwd=REPO,env=env,capture_output=True,text=True,check=True)
artifacts={}
for line in built.stdout.splitlines():
    item=json.loads(line)
    if item.get('reason')=='compiler-artifact' and item['target']['name'] in ['ef_core','ef_index','ef_config']:
        artifacts[item['target']['name']]=next(p for p in item['filenames'] if p.endswith('.rlib'))
(HERE/'probe-artifacts.json').write_text(json.dumps(artifacts,indent=2)+'\n')
with tempfile.TemporaryDirectory(prefix='echofiles-architecture-review-') as raw:
    scratch=Path(raw)
    args=['rustc','--edition=2024',str(HERE/'probe.rs'),'-L',f'dependency={TARGET}/debug/deps','-o',str(scratch/'probe')]
    for lib in ['ef_core','ef_index','ef_config']:
        args+=['--extern',f'{lib}={artifacts[lib]}']
    subprocess.run(args,check=True,cwd=REPO)
    env['XDG_DATA_HOME']=str(scratch/'data')
    subprocess.run([str(scratch/'probe'),str(scratch/'fixtures')],check=True,env=env)
