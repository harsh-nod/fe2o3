"""Retain completion/lifecycle regressions and unchanged proof/production audits."""
import hashlib
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
tests = [*cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-runtime', '--all-features', '--lib']
commands = [
    ('completion', [*tests, 'co1_']),
    ('controls', [*tests, 'async_engine::tests::owned_tests::control_tests::']),
    ('owned', [*tests, 'async_engine::tests::owned_tests::']),
    ('drain-rejection', [*tests, 'drn3a_']),
    ('terminal-retirement', [*tests, 'async_engine::tests::owned_tests::local_operation_tests::terminal_context_cannot_promote_driver_retirement', '--', '--exact']),
    ('proof-inventory', ['python3', '-I', 'crates/fe2o3-runtime-model/verus/check-negative-quality.py',
        'crates/fe2o3-runtime-model/verus/negative', 'crates/fe2o3-runtime-model/verus/verify-verus.sh']),
    ('production-metadata', [*cargo, 'metadata', '--locked', '--offline', '--no-default-features',
        '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-musl']),
    ('production-audit', ['python3', '-B', 'scripts/runtime_pure_rust_audit.py', 'metadata',
        '--input', str(root / 'r98-production-metadata.log'), '--root', 'fe2o3-runtime']),
]

def identities():
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=repo).split(b'\0')
    return {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
        for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}

before = identities()
assert before == json.loads((root / 'r98-frozen-source.json').read_text())
with (root / 'r98-auxiliary-source-inputs.json').open('x') as output:
    json.dump(before, output, indent=2)
results = []
for name, command in commands:
    log = root / f'r98-{name}.log'
    started = time.monotonic()
    with log.open('xb') as output:
        result = subprocess.run(command, cwd=repo, env=env, stdout=output,
            stderr=None if name == 'production-metadata' else subprocess.STDOUT, timeout=1800)
    row = dict(name=name, command=command, cwd=str(repo), log=str(log), returncode=result.returncode,
        elapsed_seconds=round(time.monotonic() - started, 3))
    results.append(row)
    (root / 'r98-auxiliary-results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(row), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
after = identities()
with (root / 'r98-auxiliary-source-after.json').open('x') as output:
    json.dump(after, output, indent=2)
assert after == before, 'source changed during auxiliary gates'
with (root / 'r98-auxiliary-complete.json').open('x') as output:
    json.dump(dict(completed=True, gates=len(results), source_identities=len(after)), output, indent=2)
print(f'PASS: {len(before)} auxiliary source identities unchanged', flush=True)
