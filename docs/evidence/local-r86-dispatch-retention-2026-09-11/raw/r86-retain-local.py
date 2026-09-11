"""Retain exact-source R86 data-conversion evidence, not full construction acceptance."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r86-dispatch-retention-2026-09-11'
attempt = sys.argv[1]
assert re.fullmatch(r'r86-[a-z0-9]+', attempt)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '9f8779faab169271d716f4a241f849fef0af3567'
gates = json.loads((temporary / f'{attempt}-source-gate.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
before = json.loads((temporary / f'{attempt}-source-inputs.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before, 'source changed after accepted gates'
assert len(now) == 5620
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
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2401, failed=0, ignored=5, harnesses=48)
assert summary['gnu-host'] == dict(passed=258, failed=0, ignored=4, harnesses=1)
assert summary['musl-host'] == dict(passed=141, failed=0, ignored=0, harnesses=1)
for label, passed, harnesses in [
    ('retention', 13, 1), ('readback', 10, 1), ('charged', 50, 1),
    ('shells', 19, 1), ('storage', 9, 2), ('adoption', 15, 1), ('async', 233, 1),
]:
    result = counts((temporary / f'r86-focused-{label}.log').read_text())
    assert result == dict(passed=passed, failed=0, ignored=0, harnesses=harnesses)
    summary[f'focused_{label}'] = result
for label in ('preflight', 'restored'):
    assert counts((temporary / f'r86-{label}.log').read_text()) == summary['focused_retention']
for label in ('early-move', 'duplicate'):
    log = (temporary / f'r86-{label}-mutation.log').read_text()
    assert 'test result: FAILED. 0 passed; 1 failed; 0 ignored;' in log
    if label == 'early-move':
        assert 'left: []' in log and 'InputIdentity' in log
    else:
        assert 'assertion failed: retain_v1' in log
inventory = (temporary / 'r86-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
assert (temporary / 'r86-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
helper = 'crates/fe2o3-kfd/src/shared_memory/dispatch_retention.rs'
assert now[helper] == 'bf8ba6f1f408befdfea58c5385f2b98b07ab5aba6a54a14f85207e4e7011ae1b'
summary.update(
    base_commit=base, final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    new_kfd_cpu_test_functions=13, new_runtime_cpu_test_functions=0,
    new_host_cpu_test_functions=0, new_compile_fail_doctests=0,
    whole_roster_data_conversion_implemented=True,
    original_roster_preserved_through_preflight_cpu_validated=True,
    live_session_wrapper_fixture_executed=False,
    exact_native_records_and_optional_charges_cpu_validated=True,
    five_storage_variants_cpu_validated=True,
    early_move_mutation_rejected=True, duplicate_mutation_rejected=True,
    negative_mutations_precede_unrelated_capacity_test_lint_fix=True,
    restored_helper_sha256=now[helper],
    full_native_construction_custody_implemented=False,
    code_kernarg_failure_descriptor_recovery_fixed=False,
    outer_unwind_and_closing_retake_custody_implemented=False,
    native_generated_adoption_hooks_installed=False,
    linux_data_conversion_qualified=False, adapter_refinement_proved=False,
    gpu_performance_measured=False, verus_solver_rerun=False,
    new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from='fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    manifests_and_lockfiles_unchanged=True,
    focused_storage_overlaps_shells_by_one_runtime_test=True,
    production_metadata_sha256=hashlib.sha256((temporary / 'r86-production-metadata.json').read_bytes()).hexdigest())
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r86-*.log'))
files += list(temporary.glob('r86-*-source-gate.json'))
files += list(temporary.glob('r86-*-source-inputs.json'))
files += [temporary / name for name in ['r86-source-gate.py', 'r86-auxiliary-gates.py',
    'r86-retain-local.py', 'r86-doc-links.js', 'r86-focused-check.py', 'r86-production-metadata.json']]
for source in files:
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
