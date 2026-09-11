"""Retain exact local R92 acceptance, failed attempts and compiled negative mutations."""
import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r92-primary-construction-2026-09-11'
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == 'a04060137e8df20f434682f7ee4dd6f06fb10174'
gates = json.loads((temporary / 'r92-final-source-gate.json').read_text())
auxiliary = json.loads((temporary / 'r92-auxiliary-results.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
assert len(auxiliary) == 11 and all(g['returncode'] == 0 for g in auxiliary)
before = json.loads((temporary / 'r92-final-source-inputs.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before
proof_base = 'fb1e27e66cee27bb11f0b7c08f2d5994c5d168da'
subprocess.run(git + ['diff', '--exit-code', proof_base, '--',
    'crates/fe2o3-runtime-model', 'crates/fe2o3-resource-accounting',
    'crates/fe2o3-completion', 'Cargo.toml', 'Cargo.lock'], check=True)

def counts(log, failures=0):
    rows = re.findall(r'^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', log, re.MULTILINE)
    if not rows:
        return None
    result = dict(zip(('passed', 'failed', 'ignored'),
        (sum(int(row[i]) for row in rows) for i in range(3))))
    result['harnesses'] = len(rows)
    assert result['failed'] == failures
    return result

summary = {g['name']: count for g in gates + auxiliary if (count := counts(Path(g['log']).read_text()))}
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2491, failed=0, ignored=5, harnesses=48)
assert summary['primary']['passed'] == 8 and summary['ordinary']['passed'] == 1
mutations = json.loads((temporary / 'r92-mutations.json').read_text())
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
mutation_results = []
for mutation in mutations:
    name = mutation['name']
    stem = f'r92-mutation-{name}'
    record = json.loads((temporary / f'{stem}.json').read_text())
    snapshot = json.loads((temporary / f'{stem}-source.json').read_text())
    log = (temporary / f'{stem}.log').read_text()
    assert record['source_unchanged'] and record['returncode'] == 101
    assert counts(log, 1) == dict(passed=0, failed=1, ignored=0, harnesses=1)
    assert 'Finished `test`' in log and mutation['assertion'] in log
    source = mutation['path']
    assert [name for name in now if now[name] != snapshot[name]] == [source]
    original = (repo / source).read_text()
    mutated = original
    for old, new in mutation['edits']:
        assert mutated.count(old) == 1
        mutated = mutated.replace(old, new, 1)
    assert hashlib.sha256(mutated.encode()).hexdigest() == snapshot[source]
    patch = ''.join(difflib.unified_diff(original.splitlines(keepends=True), mutated.splitlines(keepends=True),
        fromfile='a/' + source, tofile='b/' + source))
    (raw / f'{stem}.patch').write_text(patch)
    mutation_results.append(dict(name=name, source=source, assertion=mutation['assertion'],
        mutated_source_sha256=snapshot[source], patch_sha256=hashlib.sha256(patch.encode()).hexdigest()))

assert 'PASS:' in (temporary / 'r92-proof-inventory.log').read_text()
assert (temporary / 'r92-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
summary.update(base_commit=base, final_gate_attempt='r92-final',
    non_documentation_source_identities_checked=len(now), new_kfd_cpu_test_functions=13,
    native2a2_primary_root_implemented=True,
    ordinary_fixed_preparation_rooted_before_validation=True, returned_generic_preparation_rooted=True,
    full_native2a3_per_stage_integration_accepted=False, auxiliary_constructor_outer_custody=False,
    replacement_preparation_outer_custody=False, unreturned_callback_values_retained=False,
    new_bootstrap_backing_charges=False, native_generated_adoption_hooks_installed=False,
    linux_qualified=False, adapter_refinement_proved=False, gpu_performance_measured=False,
    verus_solver_rerun=False, new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from=proof_base, expected_negative_mutations=mutation_results,
    production_metadata_sha256=hashlib.sha256((temporary / 'r92-production-metadata.log').read_bytes()).hexdigest())
files = {temporary / 'r92-final-source-inputs.json', temporary / 'r92-final-source-gate.json',
    temporary / 'r92-auxiliary-results.json', temporary / 'r92-mutations.json'}
files.update(Path(g['log']) for g in gates + auxiliary)
for stem in ['r92-first-check', 'r92-first-tests', 'r92-kfd-local', 'r92-kfd-corrected',
    'r92-clippy-preflight', 'r92-mutation-cleanup', 'r92-mutation-gate', 'r92-mutation-shadow']:
    files.update(temporary / f'{stem}{suffix}' for suffix in ['.log', '.json', '-source.json'])
files.update(temporary / name for name in ['r92-run.py', 'r92-source-gate.py',
    'r92-auxiliary-gates.py', 'r92-retain-local.py', 'r92-doc-links.js'])
for source in sorted(files):
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
