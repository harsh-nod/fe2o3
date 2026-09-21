import hashlib
import importlib.util
from pathlib import Path
import subprocess

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
OUTPUT = Path(__file__).resolve().parent
SOURCE = 'cba869de8794e6baf0d0e183caf740f6d5dbc1f3'
BASE = ROOT / 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'
assert hashlib.sha256(BASE.read_bytes()).hexdigest() == 'df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7'
spec = importlib.util.spec_from_file_location('qualification_recorder', BASE)
B = importlib.util.module_from_spec(spec)
spec.loader.exec_module(B)
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == SOURCE
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT)
env = {'HOME': '/home/harsh', 'USER': 'harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
       'LANG': 'C', 'LC_ALL': 'C', 'CARGO_TARGET_DIR': '/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target',
       'CARGO_INCREMENTAL': '0', 'CARGO_BUILD_JOBS': '2', 'CARGO_TERM_COLOR': 'never',
       'RUSTUP_TOOLCHAIN': 'nightly-2026-04-03', 'RUST_TEST_THREADS': '8',
       'CARGO_PROFILE_DEV_OPT_LEVEL': '1', 'CARGO_PROFILE_DEV_DEBUG': '0',
       'CARGO_PROFILE_DEV_DEBUG_ASSERTIONS': 'true', 'CARGO_PROFILE_DEV_OVERFLOW_CHECKS': 'true',
       'CARGO_PROFILE_TEST_OPT_LEVEL': '1', 'CARGO_PROFILE_TEST_DEBUG': '0',
       'CARGO_PROFILE_TEST_DEBUG_ASSERTIONS': 'true', 'CARGO_PROFILE_TEST_OVERFLOW_CHECKS': 'true'}
rec = B.Recorder(OUTPUT / 'qualification', ROOT)
rec.run('source-before', ['git', 'diff', '--exit-code', SOURCE], 30, env=env)
rec.run('signature', ['git', '-c', 'gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers', 'verify-commit', SOURCE], 30, env=env)
for target in ['x86_64-unknown-linux-gnu', 'x86_64-unknown-linux-musl']:
    rec.run('model-' + target, ['cargo', 'test', '--locked', '-q', '-p', 'fe2o3-runtime-model',
            '--target', target], 2400, env=env)
    rec.run('runtime-' + target, ['cargo', 'test', '--locked', '-q', '-p', 'fe2o3-runtime',
            '--all-features', '--target', target, '--lib', '--example',
            'gfx942-runtime-xgmi-segments-owner-smoke'], 2400, env=env)
rec.run('no-default-features', ['cargo', 'check', '--locked', '-q', '-p', 'fe2o3-runtime',
        '--no-default-features', '--target', 'x86_64-unknown-linux-gnu', '--lib'], 1200, env=env)
rec.run('model-clippy', ['cargo', 'clippy', '--locked', '-q', '-p', 'fe2o3-runtime-model',
        '--all-targets', '--target', 'x86_64-unknown-linux-gnu', '--', '-D', 'warnings'], 1200, env=env)
rec.run('runtime-clippy', ['cargo', 'clippy', '--locked', '-q', '-p', 'fe2o3-runtime',
        '--all-features', '--target', 'x86_64-unknown-linux-gnu', '--lib', '--tests',
        '--example', 'gfx942-runtime-xgmi-segments-owner-smoke', '--', '-D', 'warnings'], 1200, env=env)
rec.run('rustfmt', ['rustfmt', '--edition', '2024', '--check', '--config', 'skip_children=true',
        'crates/fe2o3-runtime-model/src/lib.rs',
        'crates/fe2o3-runtime-model/src/context_producer_reads.rs',
        'crates/fe2o3-runtime-model/src/context_producer_reads/tests.rs'], 120, env=env)
rec.run('diff-check', ['git', 'diff', '--check', '6f5bdf823886f5b3221e9b83c63bb4beb682d53c', SOURCE], 30, env=env)
rec.run('source-after', ['git', 'diff', '--exit-code', SOURCE], 30, env=env)
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT)
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == SOURCE
B.write_json(OUTPUT / 'finished.json', {'source_commit': SOURCE, 'qualified': True,
        'context_integration': False, 'native_execution': False,
        'performance_acceptance': False, 'formal_refinement': False})
