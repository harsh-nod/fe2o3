"""Retain preparation and retained-runtime checks, proof inventory and production dependency audit."""
import os
from pathlib import Path
import subprocess

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
commands = [
    ('r89-focused-preparation.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'preparation_']),
    ('r89-focused-transitions.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'shared_memory::tests::transitions']),
    ('r89-focused-allocation.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'shared_memory::tests::allocation']),
    ('r89-focused-memory.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'shared_memory::tests::']),
    ('r89-proof-inventory.log', ['python3', '-I', 'crates/fe2o3-runtime-model/verus/check-negative-quality.py',
        'crates/fe2o3-runtime-model/verus/negative', 'crates/fe2o3-runtime-model/verus/verify-verus.sh']),
    ('r89-production-metadata.json', ['cargo', '+nightly-2026-04-03', 'metadata', '--locked', '--offline',
        '--no-default-features', '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-musl']),
    ('r89-production-audit.log', ['python3', '-B', 'scripts/runtime_pure_rust_audit.py', 'metadata',
        '--input', str(root / 'r89-production-metadata.json'), '--root', 'fe2o3-runtime']),
    ('r89-focused-retention.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'dispatch_retention']),
    ('r89-focused-readback.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-host', '--all-features', '--lib', 'reserved_readback']),
    ('r89-focused-charged.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-host', '--all-features', '--lib', 'generated_runtime_arguments::charged_tests']),
    ('r89-focused-shells.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '--all-features', '--lib', 'shell_']),
    ('r89-focused-storage.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '-p', 'fe2o3-host', '--all-features', '--lib', 'generated_storage']),
    ('r89-focused-adoption.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '--all-features', '--lib', 'adoption_tests']),
    ('r89-focused-async.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '--all-features', '--lib', 'async_engine::']),
    ('r89-doc-links.log', ['node', str(root / 'r89-doc-links.js')]),
]
for name, command in commands:
    with (root / name).open('xb') as output:
        run = subprocess.run(command, cwd=repo, env=env, stdout=output,
            stderr=None if name.endswith('.json') else subprocess.STDOUT, timeout=1800)
    print(f'{name}: {run.returncode}', flush=True)
    if run.returncode:
        raise SystemExit(run.returncode)
