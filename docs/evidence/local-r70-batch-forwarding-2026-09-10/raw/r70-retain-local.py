"""Retain R70 swarm gates, failed attempts and exact changed-source identities."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r70-batch-forwarding-2026-09-10'
raw = root / 'raw'
attempt = sys.argv[1]
assert attempt in ('r70-final', 'r70-final2', 'r70-final3')
results = json.loads((temporary / f'{attempt}-source-gate.json').read_text())
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
proof = (temporary / 'r70-verus-final.log').read_text()
positive = re.findall(r'^verification results:: (\d+) verified, 0 errors$', proof, re.MULTILINE)
negative = re.findall(r'^expected-negative rejected: (.+)$', proof, re.MULTILINE)
assert len(positive) == 59 and sum(map(int, positive)) == 1347
assert len(negative) == 659 and len(set(negative)) == 659
assert '\nFE2O3_RUNTIME_MODEL_VERUS_OK ' in proof and 'FAIL:' not in proof
assert proof.count('PASS: pinned Verus release closure matched') == 2
summary['verus'] = dict(positive_sources=59, obligations=1347, expected_negatives=659,
                        native_or_whole_executor_proof=False)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '440200f0eefef44024de019a21fbc48541c85a35'
paths = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
paths += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in paths.split(b'\0') if p
                  and not p.startswith(b'docs/evidence/')})
assert not any(p.endswith(('Cargo.toml', 'Cargo.lock')) for p in sources)
summary['base_commit'] = base
summary['dependencies_changed'] = False
summary['final_gate_attempt'] = attempt
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r70-*.log'))
files += list(temporary.glob('r70-*-source-gate.json'))
files += [temporary / name for name in ('r70-source-gate.py',
    'r70-production-metadata.json', 'r70-retain-local.py')]
for path in files:
    shutil.copyfile(path, raw / path.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(
    f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(
    f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(root)}\n'
    for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
