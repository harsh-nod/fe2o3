"""Retain exact-source CONTROL-2 evidence; do not promote CPU tests to native/proof acceptance."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r89-dispatch-preparation-2026-09-11'
attempt = sys.argv[1]
assert re.fullmatch(r'r89-[a-z0-9]+', attempt)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '0a040ece9816335fe4ad0178df9213eb84eca108'
gates = json.loads((temporary / f'{attempt}-source-gate.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
before = json.loads((temporary / f'{attempt}-source-inputs.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before, 'source changed after accepted gates'
assert len(now) == 5627
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

summary = {g['name']: count for g in gates if (count := counts(Path(g['log']).read_text()))}
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2454, failed=0, ignored=5, harnesses=48)
assert summary['gnu-host'] == dict(passed=258, failed=0, ignored=4, harnesses=1)
assert summary['musl-host'] == dict(passed=141, failed=0, ignored=0, harnesses=1)
for label, passed, harnesses in [
    ('preparation', 19, 1), ('transitions', 20, 1), ('memory', 195, 1), ('allocation', 14, 1),
    ('retention', 13, 1), ('readback', 10, 1), ('charged', 50, 1),
    ('shells', 19, 1), ('storage', 9, 2), ('adoption', 15, 1), ('async', 233, 1),
]:
    result = counts((temporary / f'r89-focused-{label}.log').read_text())
    assert result == dict(passed=passed, failed=0, ignored=0, harnesses=harnesses)
    summary[f'focused_{label}'] = result

restored = json.loads((temporary / 'r89-restored.json').read_text())
hardened = json.loads((temporary / 'r89-hardened.json').read_text())
assert restored['before'] == restored['after'] == hardened['before'] == hardened['after']
assert restored['actual'] == hardened['actual'] == 0
assert all(now[name] == digest for name, digest in restored['before'].items())
assert counts((temporary / 'r89-restored.log').read_text()) == summary['focused_preparation']
mutation_records = []
mutated_source = 'crates/fe2o3-kfd/src/queue_dispatch_binding/preparation.rs'
for label, assertion in [
    ('current-token-final', 'missing actual control owner'),
    ('content', 'left: None'),
    ('marker', 'every terminal transition must have its exact preparation handoff marker'),
]:
    prefix = f'r89-{label}-mutation'
    log = (temporary / f'{prefix}.log').read_text()
    record = json.loads((temporary / f'{prefix}.json').read_text())
    assert record['before'] == record['after'] and record['unchanged']
    assert record['actual'] == record['expected'] == 101
    assert counts(log, failures=1) == dict(passed=0, failed=1, ignored=0, harnesses=1)
    assert assertion in log
    changed = [name for name, digest in record['before'].items() if digest != now[name]]
    assert changed == [mutated_source], changed
    mutation_records.append(dict(label=label, assertion=assertion,
        source_sha256=record['before'][mutated_source],
        patch_sha256=hashlib.sha256((temporary / f'{prefix}.patch').read_bytes()).hexdigest()))

inventory = (temporary / 'r89-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
assert (temporary / 'r89-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
summary.update(
    base_commit=base, final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    new_kfd_cpu_test_functions=19, new_runtime_cpu_test_functions=0,
    new_host_cpu_test_functions=0, new_compile_fail_doctests=0,
    control2_complete_preparation_custody_implemented=True,
    one_production_sequencer_and_existing_planner=True,
    generation_precedes_first_fallible_stage=True,
    original_data_descriptors_and_actual_native_bytes_preserved=True,
    exact_control_marker_to_session_token_linkage=True,
    successful_preparation_rooted_across_persistent_construction_retake=True,
    full_control3_bind_settlement_implemented=False,
    queue_constructor_outer_custody_implemented=False,
    native_generated_adoption_hooks_installed=False,
    kernarg_executable_control_budgets_implemented=False,
    linux_control2_qualified=False, adapter_refinement_proved=False,
    gpu_performance_measured=False, verus_solver_rerun=False,
    new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from=proof_base,
    manifests_and_lockfiles_unchanged=True,
    expected_negative_mutations=mutation_records,
    restored_r89_source_sha256=restored['before'],
    focused_snapshot_scope='nine R89 Rust files, not every historical focused run',
    production_metadata_sha256=hashlib.sha256((temporary / 'r89-production-metadata.json').read_bytes()).hexdigest())
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
files = [path for path in temporary.glob('r89-*')
    if path.is_file() and path.suffix in ('.log', '.json', '.py', '.js', '.patch')]
for source in files:
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
