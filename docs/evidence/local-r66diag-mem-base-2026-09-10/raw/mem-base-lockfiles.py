"""Update only the new workspace dependency in standalone runtime lockfiles."""
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
    if not any(p['name'] == 'fe2o3-runtime' for p in packages):
        continue
    if any(p['name'] == 'fe2o3-resource-accounting' for p in packages):
        continue
    subprocess.run(['cargo', 'update', '--offline', '--manifest-path',
                    str(path.with_name('Cargo.toml')), '-p', 'fe2o3-runtime'],
                   cwd=repo, check=True, timeout=120)
    after = tomllib.loads(path.read_text())
    addition = [p for p in after['package'] if p['name'] == 'fe2o3-resource-accounting']
    assert addition == [dict(name='fe2o3-resource-accounting', version='0.1.0',
                             dependencies=['fe2o3-runtime-model'])], path
    after['package'] = [p for p in after['package'] if p not in addition]
    runtime = next(p for p in after['package'] if p['name'] == 'fe2o3-runtime')
    runtime['dependencies'].remove('fe2o3-resource-accounting')
    # Older fixtures predate existing completion/arrayvec edges. Cargo must
    # reconcile those local edges too; registry identities remain untouched.
    old_runtime = next(p for p in packages if p['name'] == 'fe2o3-runtime')
    if 'fe2o3-completion' not in old_runtime['dependencies']:
        runtime['dependencies'].remove('fe2o3-completion')
    old_kfd = next(p for p in packages if p['name'] == 'fe2o3-kfd')
    new_kfd = next(p for p in after['package'] if p['name'] == 'fe2o3-kfd')
    if 'arrayvec' not in old_kfd['dependencies']:
        new_kfd['dependencies'].remove('arrayvec')
    if not any(p['name'] == 'fe2o3-completion' for p in packages):
        completion = next(p for p in after['package'] if p['name'] == 'fe2o3-completion')
        assert completion == dict(name='fe2o3-completion', version='0.1.0',
                                  dependencies=['sha2 0.11.0'])
        after['package'].remove(completion)
    assert before == after, f'unrelated lockfile change: {path}'
    print(f'Exact MEM-BASE dependency addition: {path.relative_to(repo)}', flush=True)
    count += 1
print(f'Standalone runtime lockfiles minimally updated: {count}', flush=True)
