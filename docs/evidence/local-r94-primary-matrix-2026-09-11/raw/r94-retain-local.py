"""Retain exact local R94 acceptance, failed attempts and compiled negative mutations."""
import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r94-primary-matrix-2026-09-11'
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '56ba2556c9572d30f65faab3e579d95fa3fb7c3e'
gates = json.loads((temporary / 'r94-corrected-source-gate.json').read_text())
auxiliary = json.loads((temporary / 'r94-auxiliary-results.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
assert len(auxiliary) == 11 and all(g['returncode'] == 0 for g in auxiliary)
before = json.loads((temporary / 'r94-corrected-source-inputs.json').read_text())
assert before == json.loads((temporary / 'r94-guard-formatted-source.json').read_text())
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
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2508, failed=0, ignored=5, harnesses=48)
assert summary['primary']['passed'] == 25 and summary['ordinary']['passed'] == 1
mutations = json.loads((temporary / 'r94-corrected-mutations.json').read_text())
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
mutation_results = []
for mutation in mutations:
    name = mutation['name']
    stem = f'r94-mutation-{name}'
    record = json.loads((temporary / f'{stem}.json').read_text())
    snapshot = json.loads((temporary / f'{stem}-source.json').read_text())
    log = (temporary / f'{stem}.log').read_text()
    assert record['command'] == ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
        '-p', 'fe2o3-kfd', '--all-features', '--lib', mutation['test'], '--', '--exact']
    assert f'test {mutation["test"]} ... FAILED' in log
    assert record['source_unchanged'] and record['returncode'] == 101
    assert counts(log, 1) == dict(passed=0, failed=1, ignored=0, harnesses=1)
    assert 'Finished `test`' in log and mutation['assertion'] in log
    source = mutation['path']
    assert snapshot.keys() == now.keys()
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
    mutation_results.append(dict(name=name, source=source, test=mutation['test'], assertion=mutation['assertion'],
        mutated_source_sha256=snapshot[source], patch_sha256=hashlib.sha256(patch.encode()).hexdigest()))

assert 'PASS:' in (temporary / 'r94-proof-inventory.log').read_text()
assert (temporary / 'r94-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
restoration = json.loads((temporary / 'r94-final-mutation-check.json').read_text())
assert restoration['returncode'] == 0 and restoration['source_unchanged']
assert now == json.loads((temporary / 'r94-final-mutation-check-source.json').read_text())
failed_gates = json.loads((temporary / 'r94-final-source-gate.json').read_text())
assert len(failed_gates) == 1 and failed_gates[0]['name'] == 'gnu-tests'
assert failed_gates[0]['returncode'] == 101
assert counts((temporary / 'r94-final-gnu-tests.log').read_text(), 1) == dict(
    passed=2507, failed=1, ignored=5, harnesses=48)
summary.update(base_commit=base, final_gate_attempt='r94-corrected',
    preliminary_source_gate_attempt='r94-final', preliminary_mutation_campaign='r94-mutations.json',
    accepted_mutation_campaign='r94-corrected-mutations.json', mutation_command_identity_checked=True,
    non_documentation_source_identities_checked=len(now), new_kfd_cpu_test_functions=12,
    shared_production_primary_sequence=True, same_session_integration_cases=548,
    original_accounts_and_actual_resource_authority=True,
    scripted_kfd_event_create_and_doorbells=True, integrated_generic_returned_preparation=True,
    named_native2a3_cpu_matrix_accepted=True, local_linux_helper_composition=True, auxiliary_constructor_outer_custody=False,
    replacement_preparation_outer_custody=False, unreturned_callback_values_retained=False,
    new_bootstrap_backing_charges=False, native_generated_adoption_hooks_installed=False,
    linux_qualified=False, adapter_refinement_proved=False, gpu_performance_measured=False,
    verus_solver_rerun=False, new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from=proof_base, expected_negative_mutations=mutation_results,
    production_metadata_sha256=hashlib.sha256((temporary / 'r94-production-metadata.log').read_bytes()).hexdigest())
files = {temporary / 'r94-corrected-source-inputs.json', temporary / 'r94-corrected-source-gate.json',
    temporary / 'r94-final-source-inputs.json', temporary / 'r94-final-source-gate.json',
    temporary / 'r94-final-gnu-tests.log', temporary / 'r94-auxiliary-results.json',
    temporary / 'r94-mutations.json', temporary / 'r94-corrected-mutations.json'}
files.update(Path(g['log']) for g in gates + auxiliary)
for stem in ['r94-first-integration', 'r94-second-integration', 'r94-platform-integration',
    'r94-local-projection-integration', 'r94-composed-integration',
    'r94-clippy-preflight', 'r94-clippy-corrected', 'r94-guard-corrected', 'r94-guard-formatted',
    'r94-mutation-external-handoff', 'r94-mutation-late-outputs', 'r94-mutation-doorbell-pid',
    'r94-mutation-event-identity', 'r94-mutation-payload-cleanup', 'r94-mutation-restoration',
    'r94-corrected-mutation-restoration', 'r94-final-mutation-check',
    *[f'r94-mutation-{m["name"]}' for m in mutations]]:
    files.update(temporary / f'{stem}{suffix}' for suffix in ['.log', '.json', '-source.json'])
files.update(temporary / name for name in ['r94-run.py', 'r94-source-gate.py',
    'r94-auxiliary-gates.py', 'r94-retain-local.py', 'r94-doc-links.js',
    'r94-restoration.js', 'r94-corrected-restoration.js', 'r94-final-mutation-check.js'])
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
