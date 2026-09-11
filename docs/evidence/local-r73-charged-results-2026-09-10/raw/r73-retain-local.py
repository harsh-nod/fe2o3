"""Retain R73 gates, earlier attempts, lockfile checks and exact source identities."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r73-charged-results-2026-09-10'
raw = root / 'raw'
attempt = sys.argv[1]
assert re.fullmatch(r'r73-final[0-9]*', attempt)
results = json.loads((temporary / f'{attempt}-source-gate.json').read_text())
assert len(results) == 17 and all(r['returncode'] == 0 for r in results)
summary = {}
for result in results:
    value = Path(result['log']).read_text()
    counts = re.findall(r'^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',
                        value, re.MULTILINE)
    if counts:
        summary[result['name']] = dict(zip(('passed', 'failed', 'ignored'),
            (sum(int(row[index]) for row in counts) for index in range(3))))
        summary[result['name']]['harnesses'] = len(counts)
        assert summary[result['name']]['failed'] == 0
proof = (temporary / 'r73-verus-final.log').read_text()
positive = re.findall(r'^verification results:: (\d+) verified, 0 errors$', proof, re.MULTILINE)
negative = re.findall(r'^expected-negative rejected: (.+)$', proof, re.MULTILINE)
assert len(positive) == 62 and sum(map(int, positive)) == 1374
assert len(negative) == 686 and len(set(negative)) == 686
assert '\nFE2O3_RUNTIME_MODEL_VERUS_OK ' in proof and 'FAIL:' not in proof
assert proof.count('PASS: pinned Verus release closure matched') == 2
summary['verus'] = dict(positive_sources=62, obligations=1374, expected_negatives=686,
                        native_or_whole_executor_proof=False)
regression = (temporary / 'r73-polling-before-fix.log').read_text()
assert 'charged result polling waited for the seed lock: Err(Timeout)' in regression
assert '0 passed; 1 failed;' in regression
focused = (temporary / 'r73-host-seventh.log').read_text()
assert '31 passed; 0 failed;' in focused
summary['polling_regression'] = dict(before_fix='expected timeout after bounded cleanup',
    focused_after_fix_passed=31, blocking_or_mutex_refinement_proof=False)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '96212b87bb67eef0dc4e8f6e0137ebf3c35e2d37'
paths = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
paths += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in paths.split(b'\0') if p
                  and not p.startswith(b'docs/evidence/')})
before = json.loads((temporary / f'{attempt}-source-inputs.json').read_text())
now_paths = subprocess.check_output(git + ['ls-files', '--cached', '--others',
    '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in now_paths if p and not p.startswith(b'docs/')})}
assert now == before, 'source differs from final-gate snapshot'
added = {'fe2o3-resource-accounting', 'fe2o3-runtime-model'}
locks = [name for name in sources if name.endswith('Cargo.lock')]
assert len(locks) == 22
for name in locks:
    old = tomllib.loads(subprocess.check_output(git + ['show', f'HEAD:{name}'], text=True))
    new = tomllib.loads((repo / name).read_text())
    packages = [p for p in new['package'] if p['name'] == 'fe2o3-host']
    assert len(packages) == 1
    package = packages[0]
    assert added <= set(package['dependencies'])
    package['dependencies'] = [d for d in package['dependencies'] if d not in added]
    assert new == old, f'unrelated lockfile change: {name}'
manifest = 'crates/fe2o3-host/Cargo.toml'
old = tomllib.loads(subprocess.check_output(git + ['show', f'HEAD:{manifest}'], text=True))
new = tomllib.loads((repo / manifest).read_text())
for name in added:
    assert new['dependencies'].pop(name) == {'workspace': True}
assert new == old, 'unrelated manifest change'
summary['base_commit'] = base
summary['dependency_edges_added'] = sorted(added)
summary['lockfiles_checked'] = locks
summary['new_dependency_packages'] = 0
summary['final_gate_attempt'] = attempt
summary['non_documentation_source_identities_checked'] = len(now)
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r73-*.log'))
files += list(temporary.glob('r73-*-source-gate.json'))
files += list(temporary.glob('r73-*-source-inputs.json'))
files += [temporary / name for name in ('r73-source-gate.py',
    'r73-production-metadata.json',
    'r73-retain-local.py', 'r73-register-proofs.py', 'r73-refresh-locks.py')]
for path in files:
    shutil.copyfile(path, raw / path.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(
    f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(
    f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(root)}\n'
    for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
