"""Machine-specific signed-source CPU proof qualification; no native execution."""

import hashlib
import importlib.util
from pathlib import Path
import signal

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
OUTPUT = Path(__file__).resolve().parent
SOURCE = '85aa7f7e02f467ac298a5adc66d6ceb62f185039'
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
       'CARGO_HOME': '/home/harsh/.cargo',
       'VERUS': '/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus',
       'VERUS_TIMEOUT_SECONDS': '180'}
rec = B.Recorder(OUTPUT / 'qualification', ROOT)
proof = 'crates/fe2o3-runtime-model/verus/'


def source_bracket(phase):
    head = rec.run('head-' + phase, ['git', 'rev-parse', 'HEAD'], 30, env=env)
    status = rec.run('status-' + phase, ['git', 'status', '--porcelain'], 30, env=env)
    B.need((head / 'stdout').read_text().strip() == SOURCE
           and not (status / 'stdout').read_bytes(), 'signed source/clean tree changed')


source_bracket('before')
rec.run('signature', ['git', '-c', 'gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers', 'verify-commit', SOURCE], 30, env=env)
rec.run('syntax', ['bash', '-n', proof + 'verify-verus.sh'], 30, env=env)
rec.run('lint', ['/home/harsh/.local/bin/ruff', 'check', proof + 'check-producer-read-lifecycle.py', proof + 'check-negative-quality.py'], 60, env=env)
rec.run('diff-check', ['git', 'diff', '--check', SOURCE + '^', SOURCE], 30, env=env)
rec.run('global-verus', ['bash', proof + 'verify-verus.sh'], 14400, env=env)
source_bracket('after')
B.need(BASE.read_bytes() == data, 'recorder changed')
B.write_json(OUTPUT / 'finished.json', {'source_commit': SOURCE, 'qualified': True,
        'logical_producer_lifecycle': True, 'rust_lifecycle_refinement': False,
        'context_integration': False, 'native_execution': False,
        'performance_acceptance': False})
