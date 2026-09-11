"""Retain exact-source CONTROL-1 CPU evidence without claiming native/refinement acceptance."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r88-session-transitions-2026-09-11'
attempt = sys.argv[1]
assert re.fullmatch(r'r88-[a-z0-9]+', attempt)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '29462c524104d9bb01052376a0ef79c9ffdc3d61'
gates = json.loads((temporary / f'{attempt}-source-gate.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
before = json.loads((temporary / f'{attempt}-source-inputs.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before, 'source changed after accepted gates'
assert len(now) == 5624
subprocess.run(git + ['diff', '--exit-code', 'fb1e27e66cee27bb11f0b7c08f2d5994c5d168da', '--',
    'crates/fe2o3-runtime-model', 'crates/fe2o3-resource-accounting', 'crates/fe2o3-completion', 'Cargo.toml', 'Cargo.lock'], check=True)

def counts(log):
    rows = re.findall(r'^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', log, re.MULTILINE)
    if not rows:
        return None
    result = dict(zip(('passed', 'failed', 'ignored'), (sum(int(row[i]) for row in rows) for i in range(3))))
    result['harnesses'] = len(rows)
    assert result['failed'] == 0
    return result

summary = {g['name']: count for g in gates if (count := counts(Path(g['log']).read_text()))}
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2435, failed=0, ignored=5, harnesses=48)
assert summary['gnu-host'] == dict(passed=258, failed=0, ignored=4, harnesses=1)
assert summary['musl-host'] == dict(passed=141, failed=0, ignored=0, harnesses=1)
for label, passed, harnesses in [
    ('transitions', 20, 1), ('memory', 195, 1), ('allocation', 14, 1),
    ('retention', 13, 1), ('readback', 10, 1), ('charged', 50, 1),
    ('shells', 19, 1), ('storage', 9, 2), ('adoption', 15, 1), ('async', 233, 1),
]:
    result = counts((temporary / f'r88-focused-{label}.log').read_text())
    assert result == dict(passed=passed, failed=0, ignored=0, harnesses=harnesses)
    summary[f'focused_{label}'] = result
for label in ('preflight', 'restored'):
    assert counts((temporary / f'r88-{label}.log').read_text()) == summary['focused_transitions']
assert counts((temporary / 'r88-memory-preflight.log').read_text()) == summary['focused_memory']
for label, assertion in [
    ('output', 'called `Option::unwrap()` on a `None` value'),
    ('early-input', 'called `Option::unwrap()` on a `None` value'),
    ('revision', 'assertion failed: poisoned.get()'),
]:
    log = (temporary / f'r88-{label}-mutation.log').read_text()
    assert 'test result: FAILED. 0 passed; 1 failed; 0 ignored;' in log
    assert assertion in log
inventory = (temporary / 'r88-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
assert (temporary / 'r88-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
expected_sources = {
    'crates/fe2o3-kfd/src/shared_memory/transitions.rs': '18e8c9d885aa4356b4427435108ce7490cb3c3e7cfa62c3ba45c9554f80a6edd',
    'crates/fe2o3-kfd/src/shared_memory/tests/transitions.rs': '386b6d0c922a1c598c56cf2be640f19046aa3af9c56249172f2ba87e0b60ab6f',
    'crates/fe2o3-kfd/src/shared_memory.rs': 'a0da1dda7635a9ba04ca608aa80397426466878b059f7df5c9624e0ef1a97508',
}
for name, expected in expected_sources.items():
    assert now[name] == expected, f'mutation source changed: {name}'
summary.update(
    base_commit=base, final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    new_kfd_cpu_test_functions=20, new_runtime_cpu_test_functions=0,
    new_host_cpu_test_functions=0, new_compile_fail_doctests=0,
    modified_existing_revision_delegation_regression=True,
    control1_session_transition_custody_implemented=True,
    allocation_token_rooted_before_model_projection=True,
    native_seal_and_map_borrow_typed_input=True,
    mapped_successor_rooted_before_model_projection=True,
    exact_terminal_token_fields_and_returned_map_progress_retained=True,
    terminal_token_reconstruction_or_cleanup_api=False,
    original_control_materialization_panic_preserved=True,
    existing_n1_n2_charges_preserved_without_refund=True,
    pure_foreign_rejection_retains_legacy_consuming_semantics=True,
    arbitrary_rejected_foreign_or_terminal_input_custody=False,
    output_discard_mutation_rejected=True, early_input_move_mutation_rejected=True,
    insufficient_revision_preflight_mutation_rejected=True,
    restored_source_sha256=expected_sources,
    full_outer_preparation_custody_implemented=False,
    both_bind_retake_and_validation_custody_implemented=False,
    queue_constructor_custody_implemented=False,
    unmap_release_adapter_custody_implemented=False,
    native_generated_adoption_hooks_installed=False,
    kernarg_executable_control_backing_budgets_implemented=False,
    linux_control1_qualified=False, adapter_refinement_proved=False,
    gpu_performance_measured=False, verus_solver_rerun=False,
    new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from='fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    manifests_and_lockfiles_unchanged=True,
    production_metadata_sha256=hashlib.sha256((temporary / 'r88-production-metadata.json').read_bytes()).hexdigest())
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r88-*.log'))
files += list(temporary.glob('r88-*-source-gate.json'))
files += list(temporary.glob('r88-*-source-inputs.json'))
files += [temporary / name for name in ['r88-source-gate.py', 'r88-auxiliary-gates.py',
    'r88-retain-local.py', 'r88-doc-links.js', 'r88-focused-check.py', 'r88-production-metadata.json']]
for source in files:
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
