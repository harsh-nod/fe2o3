"""Record post-import validation without rerunning Verus or native work."""

import hashlib
import importlib.util
from pathlib import Path
import signal

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = ROOT / 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'
data = BASE.read_bytes()
assert hashlib.sha256(data).hexdigest() == 'df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7'
spec = importlib.util.spec_from_file_location('publication_recorder', BASE)
B = importlib.util.module_from_spec(spec)
exec(compile(data, str(BASE), 'exec'), B.__dict__)
for signum in B.MANAGED:
    signal.signal(signum, B.interrupted)
scripts = ['archive.py', 'archive-selftest.py', 'qualify.py', 'publication-checks.py']
snapshots = {name: (HERE / name).read_bytes() for name in scripts}
env = {'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}
rec = B.Recorder(HERE / 'publication-checks', ROOT)
rec.run('lint', ['/home/harsh/.local/bin/ruff', 'check', *[str(HERE / name) for name in scripts]], 60, env=env)
rec.run('offline-validation', ['/usr/bin/python3', '-I', '-B', str(HERE / 'archive.py'),
        '--verify-archive', str(HERE), '--repo', str(ROOT)], 120, env=env)
rec.run('adverse-self-tests', ['/usr/bin/python3', '-I', '-B', str(HERE / 'archive-selftest.py'),
        '--archive', str(HERE), '--repo', str(ROOT)], 120, env=env)
rec.run('diff-check', ['git', 'diff', '--check'], 30, env=env)
B.need(all((HERE / name).read_bytes() == value for name, value in snapshots.items()), 'publication scripts changed')
B.need(BASE.read_bytes() == data, 'recorder changed')
B.write_json(HERE / 'publication-checks/finished.json', {
    'passed': True, 'solver_rerun': False, 'native_execution': False,
    'scripts': {name: hashlib.sha256(value).hexdigest() for name, value in snapshots.items()},
})
