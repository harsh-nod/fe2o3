"""Retain exact R91 CPU source evidence, including failed attempts and compiled mutations."""
import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/local-r91-queue-handoffs-2026-09-11'
git = ['git', '-C', str(repo)]
base = subprocess.check_output(git + ['rev-parse', 'HEAD'], text=True).strip()
assert base == 'b66e52d3246a88d76eb30252b9cfe465fa2c9882'
gates = json.loads((temporary / 'r91-final-source-gate.json').read_text())
auxiliary = json.loads((temporary / 'r91-auxiliary-results.json').read_text())
assert len(gates) == 17 and all(g['returncode'] == 0 for g in gates)
assert len(auxiliary) == 8 and all(g['returncode'] == 0 for g in auxiliary)
before = json.loads((temporary / 'r91-final-source-inputs.json').read_text())
paths = subprocess.check_output(git + ['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split(b'\0')
now = {name: hashlib.sha256((repo / name).read_bytes()).hexdigest()
    for name in sorted({p.decode() for p in paths if p and not p.startswith(b'docs/')})}
assert now == before
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
assert summary['gnu-tests'] == summary['musl-tests'] == dict(passed=2478, failed=0, ignored=5, harnesses=48)
for row in auxiliary:
    count = counts(Path(row['log']).read_text())
    if count is not None:
        summary[row['name']] = count
assert summary['construction']['passed'] == 6
assert summary['initialization']['passed'] == 4

mutations = [
    ('preflight', 'queue_live/construction.rs', 'expected actual construction token', [
        ('        memory.preflight_cpu(ring)?;\n', '')]),
    ('foundation', 'queue.rs', 'called `Option::unwrap()` on a `None` value', [
        ('        self.foundation = Some(backend.take_model_foundation()?);\n'
         '        let foundation = self\n            .foundation\n            .as_ref()\n'
         '            .expect("returned foundation custody");\n'
         '        backend.authenticate_model_foundation(foundation)?;\n',
         '        let foundation = backend.take_model_foundation()?;\n'
         '        backend.authenticate_model_foundation(&foundation)?;\n'
         '        self.foundation = Some(foundation);\n'
         '        let foundation = self.foundation.as_ref().expect("returned foundation custody");\n')]),
    ('early-move', 'queue_live/construction.rs', 'assertion `left == right` failed', [
        ('        self.started = true;\n', '        self.started = true;\n        let context_save = self.context_save.take();\n'),
        ('            self.context_save.as_ref().ok_or_else(missing)?,\n', '            context_save.as_ref().ok_or_else(missing)?,\n'),
        ('            context_save: self.context_save.take().expect("validated context-save"),\n', '            context_save: context_save.expect("validated context-save"),\n')]),
]
raw = root / 'raw'
raw.mkdir(parents=True, exist_ok=True)
mutation_results = []
for name, source, assertion, edits in mutations:
    stem = f'r91-mutation-{name}'
    record = json.loads((temporary / f'{stem}.json').read_text())
    snapshot = json.loads((temporary / f'{stem}-source.json').read_text())
    log = (temporary / f'{stem}.log').read_text()
    assert record['source_unchanged'] and record['returncode'] == 101
    assert counts(log, 1) == dict(passed=0, failed=1, ignored=0, harnesses=1)
    assert 'Finished `test`' in log and assertion in log
    source = 'crates/fe2o3-kfd/src/' + source
    assert [name for name in now if now[name] != snapshot[name]] == [source]
    original = (repo / source).read_text()
    mutated = original
    for old, new in edits:
        assert mutated.count(old) == 1
        mutated = mutated.replace(old, new, 1)
    assert hashlib.sha256(mutated.encode()).hexdigest() == snapshot[source]
    patch = ''.join(difflib.unified_diff(original.splitlines(keepends=True), mutated.splitlines(keepends=True),
        fromfile='a/' + source, tofile='b/' + source))
    (raw / f'{stem}.patch').write_text(patch)
    mutation_results.append(dict(name=name, source=source, assertion=assertion,
        mutated_source_sha256=snapshot[source], patch_sha256=hashlib.sha256(patch.encode()).hexdigest()))

assert 'PASS:' in (temporary / 'r91-proof-inventory.log').read_text()
assert (temporary / 'r91-production-audit.log').read_text().startswith('pure-Rust runtime audit: OK (')
summary.update(base_commit=base, final_gate_attempt='r91-final',
    non_documentation_source_identities_checked=len(now), new_kfd_cpu_test_functions=10,
    native2a1_lower_handoffs_implemented=True, primary_and_auxiliary_callers_use_helpers=True,
    queue_constructor_outer_custody_implemented=False,
    unreturned_callback_values_retained=False, new_bootstrap_backing_charges=False,
    native_generated_adoption_hooks_installed=False, linux_qualified=False,
    adapter_refinement_proved=False, gpu_performance_measured=False,
    verus_solver_rerun=False, new_verus_theorems=0, negative_inventory_files=686,
    model_and_proof_sources_unchanged_from=proof_base, expected_negative_mutations=mutation_results,
    production_metadata_sha256=hashlib.sha256((temporary / 'r91-production-metadata.log').read_bytes()).hexdigest())
files = {temporary / 'r91-final-source-inputs.json', temporary / 'r91-final-source-gate.json',
    temporary / 'r91-auxiliary-results.json'}
files.update(Path(g['log']) for g in gates + auxiliary)
for stem in ['r91-first-construction', 'r91-second-construction', 'r91-kfd-local',
    'r91-initialization-corrected', 'r91-clippy-preflight',
    'r91-mutation-preflight', 'r91-mutation-foundation', 'r91-mutation-early-move']:
    files.update(temporary / f'{stem}{suffix}' for suffix in ['.log', '.json', '-source.json'])
files.update(temporary / name for name in ['r91-run.py', 'r91-source-gate.py',
    'r91-auxiliary-gates.py', 'r91-retain-local.py', 'r91-doc-links.js'])
for source in sorted(files):
    shutil.copyfile(source, raw / source.name)
(root / 'test-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
changed = subprocess.check_output(git + ['diff', '--name-only', '-z', 'HEAD'])
changed += subprocess.check_output(git + ['ls-files', '-o', '--exclude-standard', '-z'])
sources = sorted({p.decode() for p in changed.split(b'\0') if p and not p.startswith(b'docs/evidence/')})
(root / 'source-files.sha256').write_text(''.join(f'{hashlib.sha256((repo / name).read_bytes()).hexdigest()}  {name}\n' for name in sources))
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(root)}\n'
    for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
