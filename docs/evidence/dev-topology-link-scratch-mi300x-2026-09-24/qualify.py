#!/usr/bin/env python3
"""Capture bounded verifier tests and replay against an unchanged evidence packet."""
import importlib.util
from pathlib import Path
import signal
import sys

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('scratch_qualification', HERE / 'campaign.py')
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
for number in C.B.MANAGED:
    signal.signal(number, C.B.interrupted)

output = HERE / sys.argv[1]
rec = C.B.Recorder(output, C.REPO)
def inputs():
    return {path.name: C.H.sha(path) for path in sorted(HERE.iterdir()) if path.is_file()}

before = inputs()
C.B.write_json(output / 'inputs-before.json', before)
try:
    for name in ('test_verify', 'verify'):
        rec.run(name, ['/usr/bin/python3', '-I', '-B', str(HERE / (name + '.py'))], 180,
                env={'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'})
finally:
    after = inputs()
    C.B.write_json(output / 'inputs-after.json', after)
    C.H.need(before == after, 'unchanged replay qualification inputs')
