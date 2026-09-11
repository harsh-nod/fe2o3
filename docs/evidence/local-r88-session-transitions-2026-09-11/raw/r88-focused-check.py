"""Retain one native-memory regression selection, including an explicitly expected mutation rejection."""
import os
from pathlib import Path
import re
import subprocess
import sys

label, selection, expected = sys.argv[1:]
assert re.fullmatch(r"r88-[a-z0-9-]+", label)
assert int(expected) in (0, 101)
root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
command = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
    '-p', 'fe2o3-kfd', '--all-features', '--lib', selection]
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4')
env.pop('XDG_RUNTIME_DIR', None)
with (root / f'{label}.log').open('xb') as output:
    run = subprocess.run(command, cwd=repo, env=env, stdout=output,
        stderr=subprocess.STDOUT, timeout=1800)
print(f'{label}: expected={expected}, actual={run.returncode}', flush=True)
if run.returncode != int(expected):
    raise SystemExit(1)
