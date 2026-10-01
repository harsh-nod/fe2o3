#!/usr/bin/env python3
"""Audit the proof-only integration and replay its lightweight CI inventory."""
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
import yaml

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
OUTPUT = Path('/home/harsh/.codex-tmp/fe2o3-a2-integration-ci-signed-results-20261001')
BASE = '92f18e4338c30c960af7cdc4b5ab18e243945ca3'
CANDIDATE = '6b9e5d87c4386692f7c213a95dd786ccdf42cb45'
V = 'crates/fe2o3-runtime-model/verus/'
WORKFLOW = ROOT / '.github/workflows/runtime-component-source-guards.yml'

def git(*args):
    return subprocess.check_output(['/usr/bin/git', *args], cwd=ROOT)

assert sys.flags.isolated and sys.flags.dont_write_bytecode
head = git('rev-parse', 'HEAD').decode().strip()
git('-c', 'gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-preflight-repeat-20260929-nnNtgZnS/preflight-final/allowed-signers', 'verify-commit', head)
paths = git('diff', '--name-only', BASE, head).decode().splitlines()
assert len(paths) == 16
assert paths == git('diff', '--name-only', CANDIDATE + '^', CANDIDATE).decode().splitlines()
native_paths = ('crates/fe2o3-runtime/src', 'crates/fe2o3-runtime-model/src',
                'crates/fe2o3-resource-accounting/src', 'crates/fe2o3-kfd/src')
assert git('diff', '--exit-code', BASE, head, '--', *native_paths) == b''
replacements = {
    '0ccf7074a0d88526954c79b84966ad29191c27b6dae2c0728153cb8459c4adfc':
        '46db873f380e92c600ad8948c101130ab75397dffd5ad7d6cfa15085cd985d1a',
    'b4f4d329fdef62bf55581d22f04942f0eae17e98afd667c63bfe17fca0abacaa':
        '74c99e4ce41f591fe74e6ed9af89c5b24883af40efda7546ba62531714f476bf',
    'ee7df5e4506eebc5cfd8d2f4999f61cc49da20a3886dd113c2bd468bb02e4984':
        'f0906f6b433b206d8b605a0c8d2002c60d72524ecd506fd57557481a589ec174',
    'b7d3bd4f0e5436002800acba4c7508e8a715e3600fcf11d58bee076285af15d5':
        '581ee6037c4277b52496429368fc0de29b9fd08e2c5f88dbdbd14492c2db3f88',
    'aee7ed5b1b470c940eec6f48ce4edecdbb29ad430388c782e69845319580042c':
        '235d7d213081356f9e50c87a8feaf21d5c7c2fe02c14e5321a2b315102e6a9cf',
}
count_edits = {
    V + 'check-producer-input-fold.py': [('SOURCE_FILES = 323', 'SOURCE_FILES = 327')],
    V + 'check-producer-input-validate.py': [('SOURCE_FILES = 328', 'SOURCE_FILES = 332')],
    V + 'check-producer-input-composition.py': [('len(implementation) == 323', 'len(implementation) == 327')],
    V + 'test-producer-input-composition.py': [
        ('len(native), 323', 'len(native), 327'),
        ('len(with_schemas), 328', 'len(with_schemas), 332')],
}
source_rows = []
for path in paths:
    original = git('show', CANDIDATE + ':' + path)
    current = (ROOT / path).read_bytes()
    expected = original
    if path in count_edits:
        for old, new in replacements.items():
            expected = expected.replace(old.encode(), new.encode())
        for old, new in count_edits[path]:
            assert expected.count(old.encode()) == 1
            expected = expected.replace(old.encode(), new.encode())
    assert current == expected, path
    assert git('show', head + ':' + path) == current, path
    source_rows.append({'path': path, 'candidate_sha256': hashlib.sha256(original).hexdigest(),
                        'integrated_sha256': hashlib.sha256(current).hexdigest(),
                        'inventory_metadata_only': path in count_edits})
raw = WORKFLOW.read_bytes()
workflow = yaml.load(raw, Loader=yaml.BaseLoader)
assert workflow['permissions'] == {'contents': 'read'}
commands = [shlex.split(line) for step in workflow['jobs']['source-controls']['steps']
            if 'run' in step for line in step['run'].splitlines() if line.strip()]
assert len(commands) == 21 and all(argv[:3] == ['python3', '-I', '-B'] for argv in commands)
OUTPUT.mkdir()
rows = []
try:
    for index, argv in enumerate(commands, 1):
        print('START ' + str(index) + ': ' + shlex.join(argv), flush=True)
        result = subprocess.run(argv, cwd=ROOT, capture_output=True, timeout=180)
        for name, data in (('stdout', result.stdout), ('stderr', result.stderr)):
            (OUTPUT / (str(index).zfill(2) + '-' + name)).write_bytes(data)
        rows.append({'argv': argv, 'exit': result.returncode,
                     'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
                     'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()})
        assert result.returncode == 0, (argv, result.stdout.decode(), result.stderr.decode())
        print('PASS ' + str(index), flush=True)
    assert WORKFLOW.read_bytes() == raw
    assert git('rev-parse', 'HEAD').decode().strip() == head
finally:
    (OUTPUT / 'result.json').write_text(json.dumps({
        'head': head, 'base': BASE, 'candidate': CANDIDATE, 'source_rows': source_rows,
        'workflow_sha256': hashlib.sha256(raw).hexdigest(), 'commands': rows,
        'all_21_pass': len(rows) == 21 and all(row['exit'] == 0 for row in rows),
        'scope': 'Exact candidate proof bodies and metadata-only guard integration. Local workflow replay includes CPU C++ callback controls; no hosted CI, Rust tests, solver or GPU qualification.'
    }, sort_keys=True, indent=2) + '\n')
