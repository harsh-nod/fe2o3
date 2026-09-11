"""Retain focused regressions, unchanged proof inventory and production closure audit."""
import json
import os
from pathlib import Path
import subprocess
import time

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
cargo = ['cargo', '+nightly-2026-04-03']
tests = [*cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib']
commands = [
    ('primary', [*tests, 'queue::live::construction_primary::']),
    ('ordinary', [*tests, 'ordinary_constructor_root_']),
    ('construction', [*tests, 'queue::live::construction::']),
    ('linux-helpers', [*tests, 'queue_linux::tests::']),
    ('initialization', [*tests, 'queue::tests::initialization::']),
    ('transitions', [*tests, 'shared_memory::tests::transitions::']),
    ('preparation', [*tests, 'preparation::tests::']),
    ('bind', [*tests, 'persistent_bind_']),
    ('proof-inventory', ['python3', '-I', 'crates/fe2o3-runtime-model/verus/check-negative-quality.py',
        'crates/fe2o3-runtime-model/verus/negative', 'crates/fe2o3-runtime-model/verus/verify-verus.sh']),
    ('production-metadata', [*cargo, 'metadata', '--locked', '--offline', '--no-default-features',
        '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-musl']),
    ('production-audit', ['python3', '-B', 'scripts/runtime_pure_rust_audit.py', 'metadata',
        '--input', str(root / 'r94-production-metadata.log'), '--root', 'fe2o3-runtime']),
]
results = []
for name, command in commands:
    log = root / f'r94-{name}.log'
    started = time.monotonic()
    with log.open('xb') as output:
        result = subprocess.run(command, cwd=repo, env=env, stdout=output,
            stderr=None if name == 'production-metadata' else subprocess.STDOUT, timeout=1800)
    row = dict(name=name, command=command, log=str(log), returncode=result.returncode,
        elapsed_seconds=round(time.monotonic() - started, 3))
    results.append(row)
    (root / 'r94-auxiliary-results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(row), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)

