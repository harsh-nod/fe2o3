"""Record one focused source-snapshotted CPU gate, including expected failures."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

root = Path('/home/harsh/.codex-tmp')
repo = root / 'fe2o3-r61-execution'
name, selection = sys.argv[1:]
assert re.fullmatch(r'r90-[a-z0-9-]+', name)

def identities():
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=repo).split(b'\0')
    return {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
        for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}

before = identities()
command = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p',
    'fe2o3-kfd', '--all-features', '--lib', selection]
env = dict(os.environ, CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
started = time.monotonic()
with (root / f'{name}.log').open('xb') as output:
    result = subprocess.run(command, cwd=repo, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=1800)
after = identities()
record = dict(command=command, returncode=result.returncode,
    elapsed_seconds=round(time.monotonic() - started, 3), before=before, after=after)
with (root / f'{name}.json').open('x') as output:
    json.dump(record, output, indent=2)
    output.write('\n')
assert before == after, 'source changed during focused test'
print(json.dumps({key: value for key, value in record.items() if key not in ('before', 'after')}), flush=True)
raise SystemExit(result.returncode)
