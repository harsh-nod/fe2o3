"""CPU qualification of linear enrollment output restoration; no native work."""

import hashlib
import importlib.util
from pathlib import Path
import signal

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-enrollment-linear-output-20260921')
OUTPUT = Path(__file__).resolve().parent
SOURCE = '2202036a93c2c340536b80f3d2e907b201833789'
BASE = ROOT / 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'
data = BASE.read_bytes()
assert hashlib.sha256(data).hexdigest() == 'df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7'
spec = importlib.util.spec_from_file_location('qualification_recorder', BASE)
B = importlib.util.module_from_spec(spec)
exec(compile(data, str(BASE), 'exec'), B.__dict__)
for signum in B.MANAGED:
    signal.signal(signum, B.interrupted)
env = {'HOME': '/home/harsh', 'USER': 'harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
       'LANG': 'C', 'LC_ALL': 'C', 'RUSTUP_HOME': '/home/harsh/.rustup',
       'CARGO_HOME': '/home/harsh/.cargo', 'CARGO_BUILD_JOBS': '2'}
rec = B.Recorder(OUTPUT / 'qualification', ROOT)


def source_bracket(phase):
    head = rec.run('head-' + phase, ['git', 'rev-parse', 'HEAD'], 30, env=env)
    status = rec.run('status-' + phase, ['git', 'status', '--porcelain'], 30, env=env)
    B.need((head / 'stdout').read_text().strip() == SOURCE
           and not (status / 'stdout').read_bytes(), 'signed source/clean tree changed')


source_bracket('before')
rec.run('signature', ['git', '-c', 'gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers', 'verify-commit', SOURCE], 30, env=env)
rec.run('format', ['cargo', 'fmt', '-p', 'fe2o3-runtime-model', '--', '--check'], 180, env=env)
rec.run('tests', ['cargo', 'test', '--offline', '-p', 'fe2o3-runtime-model', '--', '--test-threads=2'], 1200, env=env)
rec.run('clippy', ['cargo', 'clippy', '--offline', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings'], 600, env=env)
rec.run('diff-check', ['git', 'diff', '--check', SOURCE + '^', SOURCE], 30, env=env)
source_bracket('after')
B.need(BASE.read_bytes() == data, 'recorder changed')
B.write_json(OUTPUT / 'finished.json', {'source_commit': SOURCE, 'qualified': True,
        'cpu_tests': True, 'formal_equivalence': False, 'native_execution': False,
        'performance_acceptance': False})
