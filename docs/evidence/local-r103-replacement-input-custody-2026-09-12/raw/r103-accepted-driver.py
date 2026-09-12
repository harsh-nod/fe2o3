"""Run a fresh full campaign with explicitly bounded test-harness concurrency."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

root = Path('/home/harsh/.codex-tmp')
env = dict(os.environ, RUST_TEST_THREADS='4')
record = {
    'attempt': 'r103-accepted',
    'reason': 'Fresh full campaign after two unchanged musl watchdog assertions failed; contention is a hypothesis, not a proven cause.',
    'environment_overrides': {'RUST_TEST_THREADS': '4', 'CARGO_BUILD_JOBS': '4',
        'CARGO_INCREMENTAL': '0', 'PYTHONDONTWRITEBYTECODE': '1'},
    'environment_removed': ['XDG_RUNTIME_DIR'],
    'available_cpus': len(os.sched_getaffinity(0)),
    'load_average_at_start': list(os.getloadavg()),
    'prior_attempt': 'r103-final',
    'test_deadlines_changed': False,
    'test_filters_added': False,
}
with (root / 'r103-accepted-environment.json').open('x') as output:
    json.dump(record, output, indent=2)
results = []
for script, args in [('r103-source-gate.py', ['r103-accepted']),
                     ('r103-auxiliary-gates.py', [])]:
    command = [sys.executable, '-B', str(root / script), *args]
    started = time.monotonic()
    result = subprocess.run(command, env=env)
    results.append({'command': command, 'returncode': result.returncode,
        'elapsed_seconds': round(time.monotonic() - started, 3)})
    with (root / 'r103-accepted-driver-results.json').open('w') as output:
        json.dump(results, output, indent=2)
    if result.returncode:
        raise SystemExit(result.returncode)
