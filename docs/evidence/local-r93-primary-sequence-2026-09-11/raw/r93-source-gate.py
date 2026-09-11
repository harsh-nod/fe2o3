"""Serial frozen-source queue-construction gates; no hardware execution."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

repo = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
root = repo.parent
attempt = sys.argv[1]
assert re.fullmatch(r'r93-[a-z0-9]+', attempt)
crates = ['fe2o3-completion', 'fe2o3-runtime-model', 'fe2o3-resource-accounting', 'fe2o3-kfd', 'fe2o3-runtime']
packages = [arg for crate in crates for arg in ('-p', crate)]
cargo = ['cargo', '+nightly-2026-04-03']
tests = [*cargo, 'test', '--locked', '--offline', '--no-fail-fast', *packages, '--all-features']
musl = ['--target', 'x86_64-unknown-linux-musl']
lint = [*cargo, 'clippy', '--locked', '--offline', *packages, '-p', 'fe2o3-host', '-p', 'fe2o3-macros']
gates = [
    ('gnu-tests', [*tests, '--all-targets'], repo),
    ('musl-tests', [*tests, '--all-targets', *musl], repo),
    ('gnu-docs', [*tests, '-p', 'fe2o3-host', '--doc'], repo),
    ('musl-docs', [*tests, '--doc', *musl], repo),
    ('musl-host-docs', [*cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-host', '--doc', *musl], repo),
    ('gnu-host', [*cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-host', '--all-features', '--lib'], repo),
    ('musl-host', [*cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-host', '--lib', *musl], repo),
    ('macro-fixtures', [*cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-macros', '--test', 'typed_kernel_fixtures'], repo),
    ('clippy-all', [*lint, '--all-features', '--all-targets', '--', '-D', 'warnings'], repo),
    ('clippy-production', [*lint, '--no-default-features', '--lib', '--', '-D', 'warnings'], repo),
    ('python', ['python3', '-B', '-m', 'unittest', 'test_run_r26_inplace_runner',
        'test_check_r40_striped', 'test_run_r40_striped_runner', 'test_run_r60_pipeline',
        'test_check_r60_pipeline', 'test_run_r61_owner', 'test_run_r62_control',
        'test_run_r63_graph', 'test_run_r65_drain_versions', 'test_run_r66_coexistence',
        'test_check_scale3_protocol', 'test_check_drain_capture', 'test_run_drain_capture'], repo / 'benchmarks/runtime_gfx942'),
    ('fmt', [*cargo, 'fmt', '--all', '--', '--check'], repo),
    ('whitespace', ['git', 'diff', '--check'], repo),
    ('dependency-policy', ['python3', '-B', 'scripts/workspace_dependency_policy.py'], repo),
    ('dependency-tests', ['python3', '-B', 'scripts/tests/workspace_dependency_policy.py'], repo),
    ('ci-test-gate', ['bash', 'scripts/tests/ci-local-test-gate.sh'], repo),
    ('standalone-lockfiles', ['bash', 'scripts/check-standalone-lockfiles.sh'], repo),
]

def identities():
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=repo).split(b'\0')
    return {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
        for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}

before = identities()
with (root / f'{attempt}-source-inputs.json').open('x') as output:
    json.dump(before, output, indent=2)
env = dict(os.environ, CARGO_BUILD_JOBS='4', CARGO_INCREMENTAL='0', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
results = []
for name, command, cwd in gates:
    log = root / f'{attempt}-{name}.log'
    started = time.monotonic()
    with log.open('xb') as output:
        run = subprocess.run(command, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=1800)
    result = dict(name=name, command=command, cwd=str(cwd), log=str(log), returncode=run.returncode,
        elapsed_seconds=round(time.monotonic() - started, 3))
    results.append(result)
    (root / f'{attempt}-source-gate.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(result), flush=True)
    if run.returncode:
        raise SystemExit(run.returncode)
assert identities() == before, 'source changed during final gates'
print(f'PASS: {len(before)} non-documentation source identities unchanged', flush=True)
