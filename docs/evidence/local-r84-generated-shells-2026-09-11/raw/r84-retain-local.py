"""Archive exact-source generated-shell evidence; native/proof acceptance remains explicit."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r84-generated-shells-2026-09-11'
attempt = sys.argv[1]
assert re.fullmatch(r'r84-[a-z0-9]+', attempt)
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == 'e7334b8f2c0d8ddf2645e6c6d9c84746da108d81'
subprocess.run(git + ['diff', '--exit-code', 'e07de3bfb87955fc885ef0c88788678ce8170aaa', base, '--',
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
focused_adoption = counts((temporary / 'r84-focused-adoption.log').read_text())
focused_async = counts((temporary / 'r84-focused-async.log').read_text())
assert focused_adoption == dict(passed=15, failed=0, ignored=0, harnesses=1)
assert focused_async == dict(passed=233, failed=0, ignored=0, harnesses=1)
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2388, failed=0, ignored=5, harnesses=48)
focused_shells = counts((temporary / 'r84-focused-shells.log').read_text())
focused_storage = counts((temporary / 'r84-focused-storage.log').read_text())
assert focused_shells == dict(passed=19, failed=0, ignored=0, harnesses=1)
assert focused_storage == dict(passed=9, failed=0, ignored=0, harnesses=2)
inventory = (temporary / 'r84-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
audit = (temporary / 'r84-production-audit.log').read_text()
assert audit.startswith('pure-Rust runtime audit: OK (')
summary.update(base_commit=base, final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    focused_shells=focused_shells,
    focused_storage=focused_storage,
    focused_storage_overlaps_shells_by_one_runtime_test=True,
    focused_adoption=focused_adoption,
    focused_async=focused_async,
    model_and_proof_sources_unchanged_from='fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    manifests_and_lockfiles_unchanged=True, verus_solver_rerun=False, new_verus_theorems=0,
    negative_inventory_files=686, new_runtime_cpu_test_functions=25, new_kfd_cpu_test_functions=0,
    new_host_cpu_test_functions=2,
    generated_shell_metadata_cpu_validated=True,
    exact_source_and_one_shot_control_cpu_validated=True,
    whole_roster_context_accounting_cpu_validated=True,
    canonical_retained_roster_disposal_cpu_validated=True,
    actual_charged_host_storage_drop_and_readback_cpu_validated=True,
    charged_r73_carrier_through_adoption_qualified=False,
    native_generated_adoption_hooks_installed=False,
    native_generated_adoption_implemented=False,
    linux_adoption_qualified=False, generated_shell_adapter_refinement_proved=False,
    gpu_performance_measured=False,
    production_metadata_sha256=hashlib.sha256((temporary / 'r84-production-metadata.json').read_bytes()).hexdigest())
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r84-*.log'))
files += list(temporary.glob('r84-*-source-gate.json'))
files += list(temporary.glob('r84-*-source-inputs.json'))
files += [temporary / name for name in ['r84-source-gate.py', 'r84-auxiliary-gates.py',
    'r84-retain-local.py', 'r84-doc-links.js', 'r84-production-metadata.json']]
for source in files:
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
