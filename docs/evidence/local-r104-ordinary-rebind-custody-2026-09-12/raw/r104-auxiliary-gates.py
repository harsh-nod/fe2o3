"""Retain focused regressions, unchanged proof inventory and production closure audit."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4', RUST_TEST_THREADS='4', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
cargo = ['cargo', '+nightly-2026-04-03']
tests = [*cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib']
commands = [
    ('registration', [*tests, 'queue_linux::primary_fixture::tests::']),
    ('ordinary', [*tests, 'ordinary_constructor_root_']),
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
        '--input', str(root / 'r104-production-metadata.log'), '--root', 'fe2o3-runtime']),
]
def identities():
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=repo).split(b'\0')
    return {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
        for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}

before = identities()
assert before == json.loads((root / 'r104-frozen-source.json').read_text())
with (root / 'r104-auxiliary-source-inputs.json').open('x') as output:
    json.dump(before, output, indent=2)
results = []
for name, command in commands:
    log = root / f'r104-{name}.log'
    started = time.monotonic()
    with log.open('xb') as output:
        result = subprocess.run(command, cwd=repo, env=env, stdout=output,
            stderr=None if name == 'production-metadata' else subprocess.STDOUT, timeout=1800)
    row = dict(name=name, command=command, cwd=str(repo), log=str(log), returncode=result.returncode,
        elapsed_seconds=round(time.monotonic() - started, 3))
    results.append(row)
    (root / 'r104-auxiliary-results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(row), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
after = identities()
with (root / 'r104-auxiliary-source-after.json').open('x') as output:
    json.dump(after, output, indent=2)
assert after == before, 'source changed during auxiliary gates'
with (root / 'r104-auxiliary-complete.json').open('x') as output:
    json.dump(dict(completed=True, gates=len(results), source_identities=len(after)), output, indent=2)
print(f'PASS: {len(before)} auxiliary source identities unchanged', flush=True)

