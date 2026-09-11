"""Retain R95's exact local custody acceptance without native or proof overclaims."""
import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r95-auxiliary-custody-2026-09-11'
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == 'cb29dc6216cdf57c76d1f952264ceed9e257c40a'
gates = json.loads((temporary / 'r95-accepted-source-gate.json').read_text())
auxiliary = json.loads((temporary / 'r95-auxiliary-results.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
assert len(auxiliary) == 12 and all(g['returncode'] == 0 for g in auxiliary)
before = json.loads((temporary / 'r95-accepted-source-inputs.json').read_text())
assert before == json.loads((temporary / 'r95-formatted-source.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before
unformatted = json.loads((temporary / 'r95-corrected-source-inputs.json').read_text())
test_path = 'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/tests.rs'
assert unformatted.keys() == now.keys()
assert [name for name in now if now[name] != unformatted[name]] == [test_path]
formatted_line = '            let scope = retained\n                .into_inner()\n                .expect("opening failure retained scope");'
unformatted_line = '            let scope = retained.into_inner().expect("opening failure retained scope");'
test_source = (repo / test_path).read_text()
assert test_source.count(formatted_line) == 1
assert hashlib.sha256(test_source.replace(formatted_line, unformatted_line).encode()).hexdigest() == unformatted[test_path]
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
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2516, failed=0, ignored=5, harnesses=48)
assert summary['auxiliary-construction'] == dict(passed=7, failed=0, ignored=0, harnesses=1)
assert summary['primary']['passed'] == 25
for check in auxiliary[:9]:
    assert summary[check['name']]['passed'] > 0, check['name']
mutations = json.loads((temporary / 'r95-mutations.json').read_text())
assert len(mutations) == 8
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
mutation_results = []
for mutation in mutations:
    name = mutation['name']
    stem = f'r95-mutation-{name}'
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

assert 'PASS:' in (temporary / 'r95-proof-inventory.log').read_text()
assert (temporary / 'r95-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
restoration = json.loads((temporary / 'r95-accepted-mutation-check.json').read_text())
assert restoration['returncode'] == 0 and restoration['source_unchanged']
assert now == json.loads((temporary / 'r95-accepted-mutation-check-source.json').read_text())
preliminary = json.loads((temporary / 'r95-final-source-gate.json').read_text())
assert len(preliminary) == 17 and all(g['returncode'] == 0 for g in preliminary)
assert counts((temporary / 'r95-final-gnu-tests.log').read_text()) == dict(
    passed=2514, failed=0, ignored=5, harnesses=48)
stopped = json.loads((temporary / 'r95-corrected-source-gate.json').read_text())
assert len(stopped) == 12 and all(g['returncode'] == 0 for g in stopped[:-1])
assert stopped[-1]['name'] == 'fmt' and stopped[-1]['returncode'] == 1
summary.update(base_commit=base,
    source_gate_count=len(gates), auxiliary_check_count=len(auxiliary),
    implementation_start_commit='d464dc442c903e6915fe6ed11dcdfff8fee5db94',
    preceding_accepted_runtime_commit='363ce6b79938def9365016c34f020a00493c1f87',
    final_gate_attempt='r95-accepted', preliminary_source_gate_attempt='r95-final',
    stopped_source_gate_attempt='r95-corrected', preliminary_compiled_campaign='r95-v2-mutations.json',
    final_source_change_exactly_one_formatting_wrap=True,
    accepted_mutation_campaign='r95-mutations.json', preliminary_mutation_campaign='r95-preliminary-mutations.json',
    mutation_command_identity_checked=True, non_documentation_source_identities_checked=len(now),
    new_kfd_cpu_test_functions=8, new_auxiliary_cpu_test_functions=7,
    rejected_vacancy_cases=11, successful_vacancy_transfers=3,
    composed_operation_retake_cases=9, composed_opening_cases=3,
    auxiliary_constructor_outer_custody=True, original_parent_and_completed_lane_retained=True,
    opening_observation_inside_root=True, journal_capacity_preflight_observation_free=True,
    synthetic_parent_without_engine=True, scripted_model_loan=True,
    actual_charged_completion_token=True, integrated_auxiliary_native2b5_accepted=False,
    replacement_preparation_outer_custody=False, unreturned_callback_values_retained=False,
    new_bootstrap_backing_charges=False, native_generated_adoption_hooks_installed=False,
    linux_qualified=False, adapter_refinement_proved=False, gpu_performance_measured=False,
    verus_solver_rerun=False, new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from=proof_base, expected_negative_mutations=mutation_results,
    production_metadata_sha256=hashlib.sha256((temporary / 'r95-production-metadata.log').read_bytes()).hexdigest())
for source in sorted(temporary.glob('r95-*')):
    if source.is_file() and source.suffix in {'.json', '.log', '.py', '.js', '.md'}:
        shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
