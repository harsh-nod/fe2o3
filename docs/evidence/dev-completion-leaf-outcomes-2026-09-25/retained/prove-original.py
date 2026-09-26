#!/usr/bin/env python3
"""Retain each development proof attempt and its exact source inputs."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import types

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
V = ROOT / 'crates/fe2o3-runtime-model/verus'
VERUS = Path('/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus')
OUT = Path(__file__).resolve().parent / sys.argv[1]
OUT.mkdir(exist_ok=False)
paths = sorted(V.glob('context_completion_reconciliation*.rs'))
paths.append(ROOT / 'crates/fe2o3-runtime/src/context/completion_reconciliation_body.rs')
before = {}
for path in paths:
    relative = path.relative_to(ROOT)
    destination = OUT / 'source' / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(path, destination)
    before[str(relative)] = hashlib.sha256(path.read_bytes()).hexdigest()
(OUT / 'inputs.json').write_text(json.dumps(before, indent=2, sort_keys=True) + '\n')
raw = (V / 'check-journal-issuance.py').read_bytes()
assert hashlib.sha256(raw).hexdigest() == '36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480'
controller = types.ModuleType('owned_outcome_proof')
controller.__file__ = str(V / 'check-journal-issuance.py')
sys.modules[controller.__name__] = controller
exec(compile(raw, controller.__file__, 'exec'), controller.__dict__)
for number in controller.SIGNALS:
    signal.signal(number, controller.interrupted)
os.chdir(ROOT)
command = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '120', str(VERUS),
           '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
           '--error-format=json', '--no-report-long-running', '--num-threads', '4',
           *sys.argv[2:], str(V / 'context_completion_reconciliation_v1.rs')]
status, stdout, stderr = controller.run_owned(command, 130, OUT / 'proof',
    dict(os.environ, VERUS_Z3_PATH=str(VERUS.parent / 'z3'), TMPDIR=str(OUT)))
after = {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}
result = dict(status=status, unchanged=before == after)
if status == 0:
    result['verification'] = json.loads(stdout)['verification-results']
(OUT / 'result.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
print(json.dumps(result), flush=True)
if stderr:
    print(stderr)
sys.exit(0 if status == 0 and before == after else 1)
