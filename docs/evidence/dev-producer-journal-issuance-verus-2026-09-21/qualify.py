"""Signed-source producer journal issuance qualification; no native execution."""

import hashlib
import importlib.util
from pathlib import Path
import signal

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
OUTPUT = Path(__file__).resolve().parent
SOURCE = '5f3b864937ce359de8c8934dbce557036d12c631'
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
rec.run('lint', ['/home/harsh/.local/bin/ruff', 'check', '--no-cache',
                proof + 'check-producer-journal-issuance.py', proof + 'check-negative-quality.py',
                proof + 'check-read-commit.py', proof + 'check-read-invariant.py',
                proof + 'check-producer-read-invariant.py', proof + 'check-producer-read-lifecycle.py'], 60, env=env)
rec.run('diff-check', ['git', 'diff', '--check', SOURCE + '^', SOURCE], 30, env=env)
rec.run('producer-campaign', ['python3', '-I', '-B', proof + 'check-producer-journal-issuance.py',
                            proof + 'context_producer_journal_issuance_v1.rs', env['VERUS'], '180',
                            str(OUTPUT / 'producer-campaign')], 5400, env=env)
rec.run('global-verus', ['bash', proof + 'verify-verus.sh'], 14400, env=env)
source_bracket('after')
B.need(BASE.read_bytes() == data, 'recorder changed')
B.write_json(OUTPUT / 'finished.json', {'source_commit': SOURCE, 'qualified': True,
        'logical_producer_journal_issuance': True, 'rust_journal_refinement': False,
        'constructor_to_pending_reachability': False, 'context_integration': False,
        'native_execution': False, 'performance_acceptance': False})
