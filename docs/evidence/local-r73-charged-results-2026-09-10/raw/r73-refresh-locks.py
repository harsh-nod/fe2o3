"""Refresh only the new host-local workspace dependency edges in tracked lockfiles."""
import copy
import json
from pathlib import Path
import subprocess
import tomllib

repo = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
paths = subprocess.check_output(['git', 'ls-files', '-z', 'Cargo.lock', '*/Cargo.lock'], cwd=repo).split(b'\0')
count = 0
for name in paths:
    if not name:
        continue
    lock = repo / name.decode()
    before = tomllib.loads(lock.read_text())
    hosts = [package for package in before['package'] if package['name'] == 'fe2o3-host']
    if not hosts:
        continue
    print(f'Checking host dependency edges: {name.decode()}', flush=True)
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--offline', '--format-version', '1', '--manifest-path',
        str(lock.with_name('Cargo.toml'))], cwd=repo))
    expected = copy.deepcopy(before)
    real_host = any(Path(package['manifest_path']).resolve() == repo / 'crates/fe2o3-host/Cargo.toml'
                    for package in metadata['packages'])
    if real_host:
        host, = [package for package in expected['package'] if package['name'] == 'fe2o3-host']
        host['dependencies'] = sorted(set(host['dependencies']) | {'fe2o3-resource-accounting', 'fe2o3-runtime-model'})
    else:
        print('Fixture stub host is unchanged', flush=True)
    after = tomllib.loads(lock.read_text())
    assert after == expected, f'unexpected lockfile changes: {lock}'
    count += int(real_host)
print(f'Only the two intended host dependency edges changed; {count} real-host lockfiles checked')
