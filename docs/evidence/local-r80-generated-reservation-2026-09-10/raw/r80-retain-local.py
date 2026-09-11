"""Archive frozen-source reservation evidence with explicit unqualified boundaries."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r80-generated-reservation-2026-09-10'
attempt = sys.argv[1]
assert re.fullmatch(r'r80-[a-z0-9]+', attempt)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '5c5133e631f186a35d9d649915e609a7ea00c2e2'
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
inventory = (temporary / 'r80-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
audit = (temporary / 'r80-production-audit.log').read_text()
assert audit.startswith('pure-Rust runtime audit: OK (')
summary.update(base_commit=base, final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    focused_async=counts((temporary / 'r80-focused-async-expanded.log').read_text()),
    focused_async_owned=counts((temporary / 'r80-focused-async.log').read_text()),
    focused_charged_host=counts((temporary / 'r80-focused-host.log').read_text()),
    model_and_proof_sources_unchanged_from='fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    manifests_and_lockfiles_unchanged=True, verus_solver_rerun=False, new_verus_theorems=0,
    negative_inventory_files=686, new_runtime_cpu_test_functions=21,
    new_host_charged_storage_test_functions=9, new_compile_fail_doctests=2,
    complete_source_join_cpu_validated=True, charged_readback_storage_cpu_validated=True,
    finite_reservation_registry_cpu_validated=True, host_protected_constructor_wiring_reviewed=True,
    genuine_native_closing_currentness_qualified=False, native_adoption_qualified=False,
    production_generated_execution_qualified=False, whole_adapter_refinement_proved=False,
    actual_charged_storage_in_protected_engine_constructor_qualified=False,
    gpu_performance_measured=False)
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r80-*.log'))
files += list(temporary.glob('r80-*-source-gate.json'))
files += list(temporary.glob('r80-*-source-inputs.json'))
files += [temporary / name for name in ['r80-source-gate.py', 'r80-auxiliary-gates.py',
    'r80-retain-local.py', 'r80-doc-links.js', 'r80-focused-async.py', 'r80-production-metadata.json']]
for source in files:
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
