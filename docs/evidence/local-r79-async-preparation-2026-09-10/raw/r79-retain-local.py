"""Retain exact finite-preparation gates without native or adapter-proof claims."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r79-async-preparation-2026-09-10'
raw = root / 'raw'
attempt = sys.argv[1]
assert re.fullmatch(r'r79-[a-z0-9]+', attempt)
results = json.loads((temporary / f'{attempt}-source-gate.json').read_text())
assert len(results) == 17 and all(r['returncode'] == 0 for r in results)
summary = {}
for result in results:
    value = Path(result['log']).read_text()
    counts = re.findall(r'^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;',
                        value, re.MULTILINE)
    if counts:
        summary[result['name']] = dict(zip(('passed', 'failed', 'ignored'),
            (sum(int(row[index]) for row in counts) for index in range(3))))
        summary[result['name']]['harnesses'] = len(counts)
        assert summary[result['name']]['failed'] == 0
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == 'aeec40eba081baff3b856ef676d970b11a40b69a'
unchanged = ['crates/fe2o3-runtime-model', 'crates/fe2o3-resource-accounting',
             'crates/fe2o3-completion', 'Cargo.toml', 'Cargo.lock']
subprocess.run(git + ['diff', '--exit-code', 'fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    '--', *unchanged], check=True)
paths = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
paths += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in paths.split(b'\0') if p
                  and not p.startswith(b'docs/evidence/')})
assert not any(name.endswith(('Cargo.toml', 'Cargo.lock')) for name in sources)
before = json.loads((temporary / f'{attempt}-source-inputs.json').read_text())
now_paths = subprocess.check_output(git + ['ls-files', '--cached', '--others',
    '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in now_paths if p and not p.startswith(b'docs/')})}
assert now == before, 'source differs from final-gate snapshot'
inventory = (temporary / 'r79-proof-inventory.log').read_text()
assert 'PASS:' in inventory and 'FAIL:' not in inventory
metadata = (temporary / 'r79-production-audit.log').read_text()
assert metadata.startswith('pure-Rust runtime audit: OK (') and 'packages=43 ' in metadata
async_log = (temporary / 'r79-async-regression-expanded.log').read_text()
assert 'test result: ok. 204 passed; 0 failed;' in async_log
summary.update(base_commit=base, final_gate_attempt=attempt,
    non_documentation_source_identities_checked=len(now),
    manifests_and_lockfiles_unchanged=True,
    model_and_proof_sources_unchanged_from='fb1e27e66cee27bb11f0b7c08f2d5994c5d168da',
    verus_solver_rerun=False, new_verus_theorems=0, negative_inventory_files=686,
    production_metadata_packages=43, new_runtime_cpu_test_functions=18,
    new_host_source_wiring_test_functions=1, focused_async_tests_passed=204,
    new_runtime_compile_fail_doctests=4, new_host_negative_binaries=0,
    new_host_compile_only_examples=1, finite_preparation_owner_cpu_validated=True,
    host_protected_constructor_source_wiring_reviewed=True,
    full_generated_projection_adoption_validated=False,
    native_bootstrap_scope_validated=False,
    production_generated_execution_validated=False, native_execution_validated=False,
    generated_charged_result_driver_integration_validated=False)
raw.mkdir(parents=True, exist_ok=True)
files = list(temporary.glob('r79-*.log'))
files += list(temporary.glob('r79-*-source-gate.json'))
files += list(temporary.glob('r79-*-source-inputs.json'))
files += [temporary / name for name in ('r79-source-gate.py', 'r79-retain-local.py',
    'r79-production-metadata.json', 'r79-auxiliary-gates.py', 'r79-doc-links.py',
    'r79-focused.py', 'r79-async-gate.py')]
for path in files:
    shutil.copyfile(path, raw / path.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'source-files.sha256').write_text(''.join(
    f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(
    f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(root)}\n'
    for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
