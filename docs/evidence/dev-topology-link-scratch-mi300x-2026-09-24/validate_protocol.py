#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import signal
import sys

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('scratch_campaign_checks', HERE / 'campaign.py')
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
for number in C.B.MANAGED:
    signal.signal(number, C.B.interrupted)
output = HERE / sys.argv[1]
rec = C.B.Recorder(output, C.REPO)
files = [HERE / name for name in ('campaign.py', 'native.py', 'test_campaign.py', 'test_cpu_binding.py', 'validate_protocol.py')]
tests = [HERE / 'test_campaign.py', HERE / 'test_cpu_binding.py']
for lane in ('dev-xgmi-currentness-attribution-mi300x-2026-09-20', 'dev-xgmi-peer-hot-mi300x-2026-09-19'):
    for name in ('test_results.py', 'test_native.py'):
        tests.append(C.REPO / 'docs/evidence' / lane / name)
    files += [C.REPO / 'docs/evidence' / lane / name for name in ('results.py', 'native.py', 'test_results.py', 'test_native.py')]
before = {str(path): C.H.sha(path) for path in files}
C.B.write_json(output / 'inputs-before.json', before)
try:
    for index, path in enumerate(tests):
        rec.run(str(index) + '-' + path.stem, ['/usr/bin/python3', '-I', '-B', str(path)], 120,
                env={'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'})
finally:
    after = {str(path): C.H.sha(path) for path in files}
    C.B.write_json(output / 'inputs-after.json', after)
    C.H.need(before == after, 'unchanged protocol qualification inputs')
