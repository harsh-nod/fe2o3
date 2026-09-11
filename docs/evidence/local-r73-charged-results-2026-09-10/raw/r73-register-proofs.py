"""Register the R73 guard proofs without changing negative-classification policy."""
import hashlib
import importlib.util
from pathlib import Path
import re
import shlex

root = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution/crates/fe2o3-runtime-model/verus')
runner_path = root / 'verify-verus.sh'
quality_path = root / 'check-negative-quality.py'
runner = runner_path.read_text()
assert 'r73_generated_result_storage_proof=' not in runner
suffixes = ('typed_half_charge', 'readonly_double_charge', 'overflow_accepted',
    'wrong_coordinate', 'shape_length_omitted', 'shape_capacity_omitted',
    'shape_access_omitted', 'zero_rejected')
rows = [('r73_generated_result_storage_proof', 'r73_generated_result_storage',
         'r73_generated_result_storage_v1.rs', 'R73_GENERATED_RESULT_STORAGE_SHA256')]
rows += [(f'negative_r73_{s}', f'negative_r73_{s}',
          f'negative/r73_generated_result_storage_v1_{s}.rs',
          f'NEGATIVE_R73_{s.upper()}_SHA256') for s in suffixes]

def insert_before(text, anchor, addition):
    assert text.count(anchor) == 1, anchor
    return text.replace(anchor, addition + anchor)

runner = insert_before(runner, 'r72_host_visible_backing_credits_proof=', ''.join(
    f'{variable}="$script_dir/{path}"\n' for variable, _, path, _ in rows))
runner = insert_before(runner, 'expected_r72_host_visible_backing_credits=', ''.join(
    f'expected_{expected}=$(read_pin "$pin_dir/{pin}")\n' for _, expected, _, pin in rows))
runner = insert_before(runner, '    check_digest "$expected_r72_host_visible_backing_credits"', ''.join(
    f'    check_digest "$expected_{expected}" "${variable}"\n' for variable, expected, _, _ in rows))
runner = insert_before(runner, '    "$r72_host_visible_backing_credits_proof" \\\n', ''.join(
    f'    "${variable}" \\\n' for variable, _, _, _ in rows))
runner = insert_before(runner, 'check_positive "$r72_host_visible_backing_credits_proof"',
    'check_positive "$r73_generated_result_storage_proof" '
    "'verification results:: 7 verified, 0 errors' r73-generated-result-storage\n")
runner = insert_before(runner, 'check_negative "$negative_r72_zero_accepted"', ''.join(
    f'check_negative "$negative_r73_{s}" mutated_{s}_v1 r73-{s.replace("_", "-")}\n' for s in suffixes))
assert runner.count('expected_negative_files=678') == 1
runner = runner.replace('expected_negative_files=678',
    'r73_generated_result_storage_obligations=7 r73_generated_result_storage_mutations=8 expected_negative_files=686')
runner_path.write_text(runner)
for _, _, path, pin in rows:
    (root / 'pins' / pin).write_text(hashlib.sha256((root / path).read_bytes()).hexdigest() + '\n')
line, = [line for line in runner.splitlines() if line.startswith('transcript=')]
transcript = shlex.split(line)[0].partition('=')[2] + '\n'
(root / 'pins/TRANSCRIPT_SHA256').write_text(hashlib.sha256(transcript.encode()).hexdigest() + '\n')

spec = importlib.util.spec_from_file_location('r73_quality', quality_path)
quality = importlib.util.module_from_spec(spec)
spec.loader.exec_module(quality)
source = quality_path.read_text()
assert source.count('EXPECTED_RUNTIME_NEGATIVE_COUNT = 678') == 1
source = source.replace('EXPECTED_RUNTIME_NEGATIVE_COUNT = 678', 'EXPECTED_RUNTIME_NEGATIVE_COUNT = 686')
authority, names, _, _ = quality.preseal_authority_assignments(runner)
source = source.replace('EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENT_COUNT = 1490',
                        f'EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENT_COUNT = {len(names)}')
for name, digest in {
    'EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENTS_SHA256': hashlib.sha256(authority.encode()).hexdigest(),
    'EXPECTED_RUNNER_SHA256': hashlib.sha256(runner.encode()).hexdigest(),
}.items():
    source, count = re.subn(r'(' + name + r' = \(\n    ")[0-9a-f]{64}("\n\))',
                          lambda match: match[1] + digest + match[2], source)
    assert count == 1
old = quality.EXPECTED_RUNNER_FUNCTION_SHA256['check_sources']
new = hashlib.sha256(quality.runner_function_source(runner, 'check_sources').encode()).hexdigest()
assert source.count(old) == 1
source = source.replace(old, new)
quality_path.write_text(source)
(root / 'pins/NEGATIVE_QUALITY_CHECKER_SHA256').write_text(hashlib.sha256(quality_path.read_bytes()).hexdigest() + '\n')
print(f'Registered 1 positive, 8 negatives, {len(names)} authority assignments')
