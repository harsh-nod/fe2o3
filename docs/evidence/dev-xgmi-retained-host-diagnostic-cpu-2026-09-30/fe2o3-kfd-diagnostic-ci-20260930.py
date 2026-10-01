#!/usr/bin/env python3
"""Execute the checked-in lightweight CI command inventory locally."""
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import sys
import yaml

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
OUTPUT = Path('/home/harsh/.codex-tmp/fe2o3-kfd-diagnostic-ci-results-20260930')
WORKFLOW = ROOT / '.github/workflows/runtime-component-source-guards.yml'
assert sys.flags.isolated and sys.flags.dont_write_bytecode
raw = WORKFLOW.read_bytes()
workflow = yaml.load(raw, Loader=yaml.BaseLoader)
assert workflow['permissions'] == {'contents': 'read'}
commands = [shlex.split(line) for step in workflow['jobs']['source-controls']['steps']
            if 'run' in step for line in step['run'].splitlines() if line.strip()]
assert len(commands) == 17 and all(argv[:3] == ['python3', '-I', '-B'] for argv in commands)
head = subprocess.check_output(['/usr/bin/git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip()
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
    assert subprocess.check_output(['/usr/bin/git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip() == head
finally:
    (OUTPUT / 'result.json').write_text(json.dumps({
        'head': head, 'workflow_sha256': hashlib.sha256(raw).hexdigest(), 'commands': rows,
        'all_17_pass': len(rows) == 17 and all(row['exit'] == 0 for row in rows),
        'scope': 'Local workflow replay, including CPU C++ callback controls; no hosted CI, Rust tests, solver or GPU qualification.'
    }, sort_keys=True, indent=2) + '\n')
