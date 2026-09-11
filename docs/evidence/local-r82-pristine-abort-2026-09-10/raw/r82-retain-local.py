"""Archive exact-source pristine-abort evidence; native/proof acceptance remains explicit."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r82-pristine-abort-2026-09-10'
attempt = sys.argv[1]
assert re.fullmatch(r'r82-[a-z0-9]+', attempt)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == '3c534e88843a8fdd64a5b3397ec6937ce3696b48'
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
inventory = (temporary / 'r82-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
audit = (temporary / 'r82-production-audit.log').read_text()
assert audit.startswith('pure-Rust runtime audit: OK (')
summary.update(base_commit=base, final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    focused_pristine_abort=counts((temporary / 'r82-focused-abort.log').read_text()),
    focused_host_backing=counts((temporary / 'r82-focused-host-backing.log').read_text()),
    model_and_proof_sources_unchanged_from='fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    manifests_and_lockfiles_unchanged=True, verus_solver_rerun=False, new_verus_theorems=0,
    negative_inventory_files=686, new_runtime_cpu_test_functions=0, new_kfd_cpu_test_functions=19,
    pristine_generation_and_complete_roster_cpu_validated=True,
    fake_native_control_disposal_and_data_debits_validated=True,
    scripted_queue_closing_and_rebind_settlement_validated=True,
    initialized_device_bytes_authenticated=False, linux_abort_rebind_qualified=False,
    actual_model_loan_retake_qualified=False, pristine_abort_adapter_refinement_proved=False,
    native_generated_adoption_implemented=False, gpu_performance_measured=False,
    production_metadata_sha256=hashlib.sha256((temporary / 'r82-production-metadata.json').read_bytes()).hexdigest())
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r82-*.log'))
files += list(temporary.glob('r82-*-source-gate.json'))
files += list(temporary.glob('r82-*-source-inputs.json'))
files += [temporary / name for name in ['r82-source-gate.py', 'r82-auxiliary-gates.py',
    'r82-retain-local.py', 'r82-doc-links.js', 'r82-production-metadata.json']]
for source in files:
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
