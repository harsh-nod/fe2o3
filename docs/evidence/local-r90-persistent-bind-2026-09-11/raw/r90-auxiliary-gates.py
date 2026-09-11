"""Retain preparation and retained-runtime checks, proof inventory and production dependency audit."""
import os
from pathlib import Path
import subprocess

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
commands = [
    ('r90-focused-loan.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib', 'live_model_custody_']),
    ('r90-focused-bind.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib', 'persistent_bind_']),
    ('r90-focused-settlement.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib', 'settlement_']),
    ('r90-focused-replay.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib', 'replay_']),
    ('r90-focused-preparation.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'preparation_']),
    ('r90-focused-transitions.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'shared_memory::tests::transitions']),
    ('r90-focused-allocation.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'shared_memory::tests::allocation']),
    ('r90-focused-memory.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'shared_memory::tests::']),
    ('r90-proof-inventory.log', ['python3', '-I', 'crates/fe2o3-runtime-model/verus/check-negative-quality.py',
        'crates/fe2o3-runtime-model/verus/negative', 'crates/fe2o3-runtime-model/verus/verify-verus.sh']),
    ('r90-production-metadata.json', ['cargo', '+nightly-2026-04-03', 'metadata', '--locked', '--offline',
        '--no-default-features', '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-musl']),
    ('r90-production-audit.log', ['python3', '-B', 'scripts/runtime_pure_rust_audit.py', 'metadata',
        '--input', str(root / 'r90-production-metadata.json'), '--root', 'fe2o3-runtime']),
    ('r90-focused-retention.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-kfd', '--all-features', '--lib', 'dispatch_retention']),
    ('r90-focused-readback.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-host', '--all-features', '--lib', 'reserved_readback']),
    ('r90-focused-charged.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-host', '--all-features', '--lib', 'generated_runtime_arguments::charged_tests']),
    ('r90-focused-shells.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '--all-features', '--lib', 'shell_']),
    ('r90-focused-storage.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '-p', 'fe2o3-host', '--all-features', '--lib', 'generated_storage']),
    ('r90-focused-adoption.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '--all-features', '--lib', 'adoption_tests']),
    ('r90-focused-async.log', ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
        'fe2o3-runtime', '--all-features', '--lib', 'async_engine::']),
    
]
for name, command in commands:
    with (root / name).open('xb') as output:
        run = subprocess.run(command, cwd=repo, env=env, stdout=output,
            stderr=None if name.endswith('.json') else subprocess.STDOUT, timeout=1800)
    print(f'{name}: {run.returncode}', flush=True)
    if run.returncode:
        raise SystemExit(run.returncode)

