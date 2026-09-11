"""Retain exact local R93 acceptance, failed attempts and compiled negative mutations."""
import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r93-primary-sequence-2026-09-11'
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '777cbefae2721bb2edd60187a666e8d84dc80c91'
gates = json.loads((temporary / 'r93-final-source-gate.json').read_text())
auxiliary = json.loads((temporary / 'r93-auxiliary-results.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
assert len(auxiliary) == 11 and all(g['returncode'] == 0 for g in auxiliary)
before = json.loads((temporary / 'r93-final-source-inputs.json').read_text())
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
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2496, failed=0, ignored=5, harnesses=48)
assert summary['primary']['passed'] == 13 and summary['ordinary']['passed'] == 1
mutations = json.loads((temporary / 'r93-mutations.json').read_text())
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
mutation_results = []
for mutation in mutations:
    name = mutation['name']
    stem = f'r93-mutation-{name}'
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

assert 'PASS:' in (temporary / 'r93-proof-inventory.log').read_text()
assert (temporary / 'r93-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
summary.update(base_commit=base, final_gate_attempt='r93-final',
    non_documentation_source_identities_checked=len(now), new_kfd_cpu_test_functions=5,
    shared_production_primary_sequence=True, same_session_integration_cases=121,
    original_accounts_and_actual_resource_authority=True,
    scripted_platform_leaves=True, integrated_generic_returned_preparation=False,
    full_native2a3_per_stage_integration_accepted=False, auxiliary_constructor_outer_custody=False,
    replacement_preparation_outer_custody=False, unreturned_callback_values_retained=False,
    new_bootstrap_backing_charges=False, native_generated_adoption_hooks_installed=False,
    linux_qualified=False, adapter_refinement_proved=False, gpu_performance_measured=False,
    verus_solver_rerun=False, new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from=proof_base, expected_negative_mutations=mutation_results,
    production_metadata_sha256=hashlib.sha256((temporary / 'r93-production-metadata.log').read_bytes()).hexdigest())
files = {temporary / 'r93-final-source-inputs.json', temporary / 'r93-final-source-gate.json',
    temporary / 'r93-auxiliary-results.json', temporary / 'r93-mutations.json'}
files.update(Path(g['log']) for g in gates + auxiliary)
for stem in ['r93-first-check', 'r93-first-integration', 'r93-second-integration',
    'r93-third-integration', 'r93-fourth-integration', 'r93-aligned-integration',
    'r93-kfd-first', 'r93-kfd-corrected', 'r93-clippy-preflight', 'r93-clippy-corrected',
    'r93-mutation-pre-doorbell', 'r93-mutation-post-doorbell', 'r93-mutation-publication',
    'r93-mutation-restoration']:
    files.update(temporary / f'{stem}{suffix}' for suffix in ['.log', '.json', '-source.json'])
files.update(temporary / name for name in ['r93-run.py', 'r93-source-gate.py',
    'r93-auxiliary-gates.py', 'r93-retain-local.py', 'r93-doc-links.js', 'r93-restoration.js'])
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
