"""Retain the entire async_engine module, including sibling budget/snapshot tests."""
import os
from pathlib import Path
import subprocess

root = Path('/home/harsh/.codex-tmp')
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4')
env.pop('XDG_RUNTIME_DIR', None)
command = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
    'fe2o3-runtime', '--all-features', '--lib', 'async_engine::']
with (root / 'r80-focused-async-expanded.log').open('xb') as output:
    result = subprocess.run(command, cwd=root / 'fe2o3-r61-execution', env=env,
        stdout=output, stderr=subprocess.STDOUT, timeout=1800)
print(f'complete async module: {result.returncode}', flush=True)
raise SystemExit(result.returncode)
