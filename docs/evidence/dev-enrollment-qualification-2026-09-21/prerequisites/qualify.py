"""Qualify enrollment prerequisites after the inherited reader proof change."""

import hashlib
import importlib.util
import json
from pathlib import Path
import signal

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-enrollment-linear-output-20260921')
OUTPUT = Path(__file__).resolve().parent
SOURCE = '3278dbb91b77d0417b32241de0c5e41190f44ae4'
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
       'CARGO_HOME': '/home/harsh/.cargo'}
rec = B.Recorder(OUTPUT / 'qualification', ROOT)
checker = 'crates/fe2o3-runtime-model/verus/check-journal-enrollment.py'


def source_bracket(phase):
    head = rec.run('head-' + phase, ['git', 'rev-parse', 'HEAD'], 30, env=env)
    status = rec.run('status-' + phase, ['git', 'status', '--porcelain'], 30, env=env)
    B.need((head / 'stdout').read_text().strip() == SOURCE
           and not (status / 'stdout').read_bytes(), 'signed source/clean tree changed')


source_bracket('before')
rec.run('signature', ['git', '-c', 'gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers', 'verify-commit', SOURCE], 30, env=env)
rec.run('lint', ['/home/harsh/.local/bin/ruff', 'check', '--no-cache', checker], 60, env=env)
rec.run('diff-check', ['git', 'diff', '--check', SOURCE + '^', SOURCE], 30, env=env)
rec.run('enrollment-campaign', ['python3', '-I', '-B', checker, '--verus',
        '/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus',
        '--timeout', '180', '--output', str(OUTPUT / 'campaign')], 2400, env=env)
source_bracket('after')
B.need(BASE.read_bytes() == data, 'recorder changed')
report = json.loads((OUTPUT / 'campaign/report.json').read_text())
B.need(report == {'scope': 'admission prefix and conditional commit suffix only',
       'positive_obligations': 169, 'inherited_obligations': 156, 'negative_cases': 7,
       'full_batch_enrollment_verified': False, 'rust_sorting_search_refined': False,
       'native_or_performance_acceptance': False}, 'exact prerequisite-only result')
B.write_json(OUTPUT / 'finished.json', {'source_commit': SOURCE, 'qualified': True,
        'enrollment_prerequisites': True, 'full_batch_enrollment_verified': False,
        'native_execution': False, 'performance_acceptance': False})
