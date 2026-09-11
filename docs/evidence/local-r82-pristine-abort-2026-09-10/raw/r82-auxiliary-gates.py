"""Retain pristine-abort focused checks, proof inventory and production dependency audit."""
import os
from pathlib import Path
import subprocess

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
commands = [
    ('r82-proof-inventory.log', ['python3', '-I', 'crates/fe2o3-runtime-model/verus/check-negative-quality.py',
        'crates/fe2o3-runtime-model/verus/negative', 'crates/fe2o3-runtime-model/verus/verify-verus.sh']),
    ('r82-production-metadata.json', ['cargo', '+nightly-2026-04-03', 'metadata', '--locked', '--offline',
        '--no-default-features', '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-musl']),
    ('r82-production-audit.log', ['python3', '-B', 'scripts/runtime_pure_rust_audit.py', 'metadata',
        '--input', str(root / 'r82-production-metadata.json'), '--root', 'fe2o3-runtime']),
    ('r82-focused-abort.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'pristine']),
    ('r82-focused-host-backing.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'shared_memory::tests::host_backing']),
    ('r82-doc-links.log', ['node', str(root / 'r82-doc-links.js')]),
]
for name, command in commands:
    with (root / name).open('xb') as output:
        run = subprocess.run(command, cwd=repo, env=env, stdout=output,
            stderr=None if name.endswith('.json') else subprocess.STDOUT, timeout=1800)
    print(f'{name}: {run.returncode}', flush=True)
    if run.returncode:
        raise SystemExit(run.returncode)
