"""Retain one native-memory regression selection, including an explicitly expected mutation rejection."""
import os
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

label, selection, expected = sys.argv[1:]
assert re.fullmatch(r"r89-[a-z0-9-]+", label)
assert int(expected) in (0, 101)
root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
command = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
    '-p', 'fe2o3-kfd', '--all-features', '--lib', selection]
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4')
env.pop('XDG_RUNTIME_DIR', None)
sources = [
    'crates/fe2o3-kfd/src/persistent_compute.rs',
    'crates/fe2o3-kfd/src/queue_dispatch_binding.rs',
    'crates/fe2o3-kfd/src/queue_dispatch_binding/pristine_abort.rs',
    'crates/fe2o3-kfd/src/queue_dispatch_binding/preparation.rs',
    'crates/fe2o3-kfd/src/queue_dispatch_binding/preparation_tests.rs',
    'crates/fe2o3-kfd/src/queue_live.rs',
    'crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs',
    'crates/fe2o3-kfd/src/shared_memory.rs',
    'crates/fe2o3-kfd/src/shared_memory/tests/preparation.rs',
]
def snapshot():
    return {path: hashlib.sha256((repo / path).read_bytes()).hexdigest()
            for path in sources}
before = snapshot()
with (root / f'{label}.log').open('xb') as output:
    run = subprocess.run(command, cwd=repo, env=env, stdout=output,
        stderr=subprocess.STDOUT, timeout=1800)
after = snapshot()
record = dict(command=command, expected=int(expected), actual=run.returncode,
    helper_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    source_scope='nine R89 Rust files, not the full source-gate inventory',
    before=before, after=after, unchanged=before == after)
with (root / f'{label}.json').open('x') as output:
    json.dump(record, output, sort_keys=True, indent=2)
    output.write('\n')
print(f'{label}: expected={expected}, actual={run.returncode}', flush=True)
if run.returncode != int(expected) or before != after:
    raise SystemExit(1)
