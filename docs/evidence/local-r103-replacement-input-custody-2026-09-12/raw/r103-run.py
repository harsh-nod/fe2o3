"""Retain a bounded command and exact non-documentation source identities."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

repo = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
root = repo.parent
name, *command = sys.argv[1:]
assert re.fullmatch(r'r103-[a-z0-9-]+', name)
assert command

def identities():
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=repo).split(b'\0')
    return {p: hashlib.sha256((repo / p).read_bytes()).hexdigest()
        for p in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}

before = identities()
source = root / f'{name}-source.json'
with source.open('x') as output:
    json.dump(before, output, indent=2)
env = dict(os.environ, CARGO_BUILD_JOBS='4', CARGO_INCREMENTAL='0', PYTHONDONTWRITEBYTECODE='1')
env.pop('XDG_RUNTIME_DIR', None)
log = root / f'{name}.log'
started = time.monotonic()
timed_out = False
with log.open('xb') as output:
    try:
        result = subprocess.run(command, cwd=repo, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=1800)
        returncode = result.returncode
    except subprocess.TimeoutExpired:
        timed_out = True
        returncode = 124
after = identities()
record = dict(command=command, cwd=str(repo), log=str(log), source=str(source),
    source_unchanged=before == after, returncode=returncode, timed_out=timed_out,
    elapsed_seconds=round(time.monotonic() - started, 3))
with (root / f'{name}.json').open('x') as output:
    json.dump(record, output, indent=2)
print(json.dumps(record), flush=True)
print(log.read_text()[-12000:], flush=True)
raise SystemExit(returncode)
