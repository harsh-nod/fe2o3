"""Archive exact-source charged-readback evidence; native/proof acceptance remains explicit."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r85-charged-readback-2026-09-11'
attempt = sys.argv[1]
assert re.fullmatch(r'r85-[a-z0-9]+', attempt)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == 'f346aab2e958725291229eae3fffaee127230112'
subprocess.run(git + ['diff', '--exit-code', 'f346aab2e958725291229eae3fffaee127230112', base, '--',
    '.', ':(exclude)docs'], check=True)
results = json.loads((temporary / f'{attempt}-source-gate.json').read_text())
assert len(results) == 17 and all(r['returncode'] == 0 for r in results)
before = json.loads((temporary / f'{attempt}-source-inputs.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before, 'source changed after accepted gates'
subprocess.run(git + ['diff', '--exit-code', 'fb1e27e66cee27bb11f0b7c08f2d5994c5d168da', '--',
    'crates/fe2o3-runtime-model', 'crates/fe2o3-resource-accounting', 'crates/fe2o3-completion', 'Cargo.toml', 'Cargo.lock'], check=True)

def counts(log):
    rows = re.findall(r'^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', log, re.MULTILINE)
    if not rows:
        return None
    totals = dict(zip(('passed', 'failed', 'ignored'), (sum(int(row[i]) for row in rows) for i in range(3))))
    totals['harnesses'] = len(rows)
    assert totals['failed'] == 0
    return totals

summary = {result['name']: count for result in results if (count := counts(Path(result['log']).read_text()))}
focused_adoption = counts((temporary / 'r85-focused-adoption.log').read_text())
focused_async = counts((temporary / 'r85-focused-async.log').read_text())
assert focused_adoption == dict(passed=15, failed=0, ignored=0, harnesses=1)
assert focused_async == dict(passed=233, failed=0, ignored=0, harnesses=1)
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2388, failed=0, ignored=5, harnesses=48)
focused_shells = counts((temporary / 'r85-focused-shells.log').read_text())
focused_storage = counts((temporary / 'r85-focused-storage.log').read_text())
assert focused_shells == dict(passed=19, failed=0, ignored=0, harnesses=1)
assert focused_storage == dict(passed=9, failed=0, ignored=0, harnesses=2)
inventory = (temporary / 'r85-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
audit = (temporary / 'r85-production-audit.log').read_text()
assert audit.startswith('pure-Rust runtime audit: OK (')
focused_readback = counts((temporary / 'r85-focused-readback.log').read_text())
focused_charged = counts((temporary / 'r85-focused-charged.log').read_text())
assert focused_readback == dict(passed=10, failed=0, ignored=0, harnesses=1)
assert focused_charged == dict(passed=50, failed=0, ignored=0, harnesses=1)
assert summary['gnu-host'] == dict(passed=258, failed=0, ignored=4, harnesses=1)
assert summary['musl-host'] == dict(passed=141, failed=0, ignored=0, harnesses=1)
for label in ('r85-preflight', 'r85-restored'):
    assert counts((temporary / f'{label}.log').read_text()) == focused_readback
mutation_log = (temporary / 'r85-gate-mutation.log').read_text()
assert 'foreign decode' in mutation_log
assert 'test result: FAILED. 0 passed; 1 failed; 0 ignored;' in mutation_log
readback_path = 'crates/fe2o3-host/src/generated_runtime_arguments/readback.rs'
assert now[readback_path] == '781f1612e05e0e52e45ae8be80fc6c5e65eb1741e8686c96da6150a83148f2df'
summary.update(base_commit=base, focused_readback=focused_readback, focused_charged=focused_charged,
    gate_identity_mutation_rejected=True,
    gate_mutation_source_restored_sha256=now[readback_path], final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    focused_shells=focused_shells,
    focused_storage=focused_storage,
    focused_storage_overlaps_shells_by_one_runtime_test=True,
    focused_adoption=focused_adoption,
    focused_async=focused_async,
    model_and_proof_sources_unchanged_from='fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    manifests_and_lockfiles_unchanged=True, verus_solver_rerun=False, new_verus_theorems=0,
    negative_inventory_files=686, new_runtime_cpu_test_functions=0, new_kfd_cpu_test_functions=0,
    new_host_cpu_test_functions=10, new_runtime_compile_fail_doctests=0,
    private_host_completion_substrate_implemented=True,
    actual_reserved_readback_decode_cpu_validated=True,
    explicit_overlap_and_read_only_settlement_cpu_validated=True,
    gate_identity_and_full_read_only_roster_cpu_validated=True,
    original_storage_before_readiness_cpu_validated=True,
    charged_r73_carrier_through_adoption_qualified=False,
    native_generated_adoption_hooks_installed=False,
    native_generated_adoption_implemented=False,
    linux_adoption_qualified=False, charged_readback_adapter_refinement_proved=False,
    gpu_performance_measured=False,
    production_metadata_sha256=hashlib.sha256((temporary / 'r85-production-metadata.json').read_bytes()).hexdigest())
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r85-*.log'))
files += list(temporary.glob('r85-*-source-gate.json'))
files += list(temporary.glob('r85-*-source-inputs.json'))
files += [temporary / name for name in ['r85-source-gate.py', 'r85-auxiliary-gates.py',
    'r85-retain-local.py', 'r85-doc-links.js', 'r85-focused-check.py', 'r85-production-metadata.json']]
for source in files:
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
