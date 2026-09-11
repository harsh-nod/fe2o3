"""Retain frozen-source CONTROL-3 evidence without native or proof overclaims."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r90-persistent-bind-2026-09-11'
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '287f2b77be5eb8de333f64e3f73b1f892970ad1e'
gates = json.loads((temporary / 'r90-accepted-source-gate.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
before = json.loads((temporary / 'r90-accepted-source-inputs.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before and len(now) == 5629
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
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2468, failed=0, ignored=5, harnesses=48)
for log in sorted(temporary.glob('r90-focused-*.log')):
    count = counts(log.read_text())
    assert count is not None
    summary[log.stem] = count
restored = json.loads((temporary / 'r90-restored.json').read_text())
assert restored['before'] == restored['after'] == now and restored['returncode'] == 0
assert counts((temporary / 'r90-restored.log').read_text()) == dict(passed=888, failed=0, ignored=0, harnesses=1)
mutations = []
for label, source, assertion in [
    ('first-panic', 'queue_live/model_loan.rs', 'Some("retake")'),
    ('terminal-retry', 'queue_live.rs', 'left: true'),
    ('early-move', 'shared_memory/dispatch_retention.rs', 'original replay token remains rooted'),
]:
    prefix = f'r90-{label}-mutation'
    record = json.loads((temporary / f'{prefix}.json').read_text())
    log = (temporary / f'{prefix}.log').read_text()
    assert record['before'] == record['after'] and record['returncode'] == 101
    assert counts(log, 1) == dict(passed=0, failed=1, ignored=0, harnesses=1)
    assert assertion in log, (label, assertion)
    source = 'crates/fe2o3-kfd/src/' + source
    assert [name for name in now if now[name] != record['before'][name]] == [source]
    mutations.append(dict(label=label, source=source, assertion=assertion,
        source_sha256=record['before'][source],
        patch_sha256=hashlib.sha256((temporary / f'{prefix}.patch').read_bytes()).hexdigest()))
assert 'PASS:' in (temporary / 'r90-proof-inventory.log').read_text()
assert (temporary / 'r90-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
summary.update(
    base_commit=base, final_gate_attempt='r90-accepted',
    non_documentation_source_identities_checked=len(now), new_kfd_cpu_test_functions=14,
    new_runtime_cpu_test_functions=0, new_host_cpu_test_functions=0,
    control3_bind_settlement_implemented=True,
    first_operation_panic_preserved=True, exact_initial_and_replay_owners_retained=True,
    terminal_dominant_retry_for_self_owned_admitted_inputs=True,
    bind_installation_nonfallible_under_exclusive_empty_slot_invariant=True,
    replay_single_loan_no_control_rebuild=True,
    retake_and_final_validation_faults_are_scripted=True,
    replay_completion_is_model_only=True, early_ledger_native_leases_are_synthetic=True,
    queue_constructor_outer_custody_implemented=False, arbitrary_owning_loan_output_protected=False,
    native_generated_adoption_hooks_installed=False, linux_control3_qualified=False,
    adapter_refinement_proved=False, gpu_performance_measured=False,
    verus_solver_rerun=False, new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from=proof_base, manifests_and_lockfiles_unchanged=True,
    expected_negative_mutations=mutations,
    production_metadata_sha256=hashlib.sha256((temporary / 'r90-production-metadata.json').read_bytes()).hexdigest())
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
for source in sorted(temporary.glob('r90-*')):
    if source.is_file() and source.suffix in ('.log', '.json', '.py', '.js', '.patch'):
        shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
