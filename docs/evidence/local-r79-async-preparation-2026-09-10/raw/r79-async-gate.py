"""Retain the complete CPU async-engine regression gate before release."""
import os
from pathlib import Path
import subprocess
import sys

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4')
env.pop('XDG_RUNTIME_DIR', None)
command = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
           '-p', 'fe2o3-runtime', '--all-features', '--lib', 'async_engine::']
suffix = f'-{sys.argv[1]}' if len(sys.argv) > 1 else ''
with (root / f'r79-async-regression{suffix}.log').open('xb') as output:
    run = subprocess.run(command, cwd=repo, env=env, stdout=output,
                         stderr=subprocess.STDOUT, timeout=1800)
print(f'async-engine regression: exit {run.returncode}', flush=True)
raise SystemExit(run.returncode)
