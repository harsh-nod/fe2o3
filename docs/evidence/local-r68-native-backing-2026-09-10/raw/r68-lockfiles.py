"""Reconcile only the native shared-credit edge, checking structured lock records."""
from pathlib import Path
import subprocess
import tomllib

repo = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
paths = subprocess.check_output(['git', 'ls-files', '-z', '--', '*/Cargo.lock'], cwd=repo)
count = 0
for raw in paths.split(b'\0'):
    if not raw:
        continue
    path = repo / raw.decode()
    before = tomllib.loads(path.read_text())
    packages = before['package']
    old_kfd = next((p for p in packages if p['name'] == 'fe2o3-kfd'), None)
    if old_kfd is None or 'fe2o3-resource-accounting' in old_kfd.get('dependencies', []):
        continue
    subprocess.run(['cargo', 'update', '--offline', '--manifest-path',
                    str(path.with_name('Cargo.toml')), '-p', 'fe2o3-kfd'],
                   cwd=repo, check=True, timeout=120)
    after = tomllib.loads(path.read_text())
    native = next(p for p in after['package'] if p['name'] == 'fe2o3-kfd')
    native['dependencies'].remove('fe2o3-resource-accounting')
    if not any(p['name'] == 'fe2o3-resource-accounting' for p in packages):
        addition = next(p for p in after['package'] if p['name'] == 'fe2o3-resource-accounting')
        assert addition == dict(name='fe2o3-resource-accounting', version='0.1.0',
                                dependencies=['fe2o3-runtime-model']), path
        after['package'].remove(addition)
    assert before == after, f'unrelated lockfile change: {path}'
    print(f'Exact native credit edge: {path.relative_to(repo)}', flush=True)
    count += 1
print(f'Standalone native lockfiles minimally updated: {count}', flush=True)
