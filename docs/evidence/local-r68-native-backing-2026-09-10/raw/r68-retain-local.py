"""Retain MEM-2A gates, failures, exact source identities and lockfile audit."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r68-native-backing-2026-09-10'
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
results = json.loads((temporary / 'r68-final3-source-gate.json').read_text())
assert len(results) == 16 and all(r['returncode'] == 0 for r in results)
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
proof = (temporary / 'r68-verus-final.log').read_text()
positive = re.findall(r'^verification results:: (\d+) verified, 0 errors$', proof, re.MULTILINE)
negative = re.findall(r'^expected-negative rejected: (.+)$', proof, re.MULTILINE)
assert len(positive) == 57 and sum(map(int, positive)) == 1334
assert len(negative) == 645 and len(set(negative)) == 645
assert '\nFE2O3_RUNTIME_MODEL_VERUS_OK ' in proof and 'FAIL:' not in proof
assert proof.count('PASS: pinned Verus release closure matched') == 2
summary['verus'] = dict(positive_sources=57, obligations=1334, expected_negatives=645,
                        native_extraction_or_whole_executor_proof=False)
files = list(temporary.glob('r68-*.log'))
files += [temporary / name for name in ('r68-initial-source-gate.json',
    'r68-final-source-gate.json', 'r68-final2-source-gate.json', 'r68-final3-source-gate.json',
    'r68-source-gate.py', 'r68-lockfiles.py',
    'r68-production-metadata.json', 'r68-retain-local.py')]
for path in files:
    shutil.copyfile(path, raw / path.name)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == 'd65245cb362a796d737131c6fa63f65a61ec3c2c'
paths = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
paths += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in paths.split(b'\0') if p
                  and not p.startswith(b'docs/evidence/')})
lockfiles = [name for name in sources if name.endswith('Cargo.lock')]
for name in lockfiles:
    before = tomllib.loads(subprocess.check_output(git + ['show', 'HEAD:' + name], text=True))
    after = tomllib.loads((repo / name).read_text())
    native = next(p for p in after['package'] if p['name'] == 'fe2o3-kfd')
    native['dependencies'].remove('fe2o3-resource-accounting')
    assert before == after, name
summary['lockfiles'] = dict(updated=len(lockfiles), only_native_credit_dependency_added=True,
                          registry_package_records_unchanged=True, locked_standalone_manifests=32)
summary['base_commit'] = base
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(
    f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(
    f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(root)}\n'
    for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
