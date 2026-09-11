"""Retain finite owner-local preparation CPU gates, without native qualification."""
import os
from pathlib import Path
import subprocess
import sys

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4')
env.pop('XDG_RUNTIME_DIR', None)
command = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
           '--no-fail-fast', '-p', 'fe2o3-runtime', '-p', 'fe2o3-host',
           '--all-features', '--lib', 'preparation']
log = root / f'r79-focused-{sys.argv[1]}.log'
with log.open('xb') as output:
    result = subprocess.run(command, cwd=repo, env=env, stdout=output,
                            stderr=subprocess.STDOUT, timeout=1800)
print(f'{log}: exit {result.returncode}')
raise SystemExit(result.returncode)
