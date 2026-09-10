"""Stage one signed qualification campaign in a task-owned shared-host directory."""
import json
from pathlib import Path
import re
import subprocess

root = Path('/home/harsh/.codex-tmp')
commit = 'bd8aa3ded708ecd6e799a803bee8e9c7ccd6c91a'
ssh = ['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'mi300x']
stage = subprocess.check_output(ssh + ['mktemp -d /dev/shm/fe2o3-r66-owner.XXXXXXXX'], text=True).strip()
assert re.fullmatch(r'/dev/shm/fe2o3-r66-owner\.[A-Za-z0-9]+', stage)
record = {'source_commit': commit, 'stage': stage, 'phase': 'transfer'}
status = root / 'r66-remote-controller.json'
status.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record), flush=True)
files = [
    (root / 'r66-bd8aa3de-source.bundle', 'source.bundle'),
    (root / 'fe2o3-r66-owner-remote.sh', 'wrapper.sh'),
    (root / 'fe2o3-r60-trusted-allowed-signers', 'allowed-signers'),
    (Path('/home/harsh/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/futures-executor-0.3.34.crate'), 'futures-executor-0.3.34.crate'),
    (Path('/home/harsh/.cargo/registry/index/index.crates.io-1949cf8c6b5b557f/.cache/fu/tu/futures-executor'), 'futures-executor-index'),
]
try:
    for source, name in files:
        subprocess.run(['scp', '-q', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10',
                        str(source), f'mi300x:{stage}/{name}'], check=True, timeout=300)
except BaseException:
    subprocess.run(ssh + [f'find {stage} -depth -delete'], check=True, timeout=60)
    raise
record['phase'] = 'running'
status.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record), flush=True)
with (root / 'r66-bd8aa3de-capture.tar').open('wb') as capture, (root / 'r66-remote-transport.log').open('wb') as errors:
    run = subprocess.run(ssh + [f'bash {stage}/wrapper.sh {stage} {commit}'],
                         stdout=capture, stderr=errors, timeout=1800)
record.update(phase='ssh-finished', returncode=run.returncode)
status.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record), flush=True)
raise SystemExit(run.returncode)
