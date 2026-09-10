"""Register the reviewed R72 proof roster and refresh its exact audit pins."""
import hashlib
import importlib.util
from pathlib import Path
import re
import shlex

root = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution/crates/fe2o3-runtime-model/verus')
runner_path = root / 'verify-verus.sh'
quality_path = root / 'check-negative-quality.py'
runner = runner_path.read_text()
assert 'r72_host_visible_backing_credits_proof=' not in runner
suffixes = ('zero_accepted', 'oversized_accepted', 'logical_bytes_substitution',
    'gpu_view_double_charge', 'allocation_record_omitted', 'short_mapping_accepted',
    'excess_padding_accepted', 'unaligned_mapping_accepted', 'nonordinary_view_accepted')
rows = [('r72_host_visible_backing_credits_proof', 'r72_host_visible_backing_credits',
         'r72_host_visible_backing_credits_v1.rs', 'R72_HOST_VISIBLE_BACKING_CREDITS_SHA256')]
rows += [(f'negative_r72_{s}', f'negative_r72_{s}',
          f'negative/r72_host_visible_backing_credits_v1_{s}.rs',
          f'NEGATIVE_R72_{s.upper()}_SHA256') for s in suffixes]

def insert_before(text, anchor, addition):
    assert text.count(anchor) == 1, anchor
    return text.replace(anchor, addition + anchor)

runner = insert_before(runner, 'r71_device_pool_proof=', ''.join(
    f'{variable}="$script_dir/{path}"\n' for variable, _, path, _ in rows))
runner = insert_before(runner, 'expected_r71_device_pool=', ''.join(
    f'expected_{expected}=$(read_pin "$pin_dir/{pin}")\n'
    for _, expected, _, pin in rows))
runner = insert_before(runner, '    check_digest "$expected_r71_device_pool"', ''.join(
    f'    check_digest "$expected_{expected}" "${variable}"\n'
    for variable, expected, _, _ in rows))
runner = insert_before(runner, '    "$r71_device_pool_proof" \\\n', ''.join(
    f'    "${variable}" \\\n' for variable, _, _, _ in rows))
runner = insert_before(runner, 'check_positive "$r71_device_pool_proof"',
    'check_positive "$r72_host_visible_backing_credits_proof" '
    "'verification results:: 5 verified, 0 errors' r72-host-visible-backing-credits\n")
runner = insert_before(runner, 'check_negative "$negative_r71_logical_bytes_undercharge"', ''.join(
    f'check_negative "$negative_r72_{s}" mutated_{s}_v1 r72-{s.replace("_", "-")}\n'
    for s in suffixes))
assert runner.count('expected_negative_files=669') == 1
runner = runner.replace('expected_negative_files=669',
    'r72_host_visible_backing_credits_obligations=5 '
    'r72_host_visible_backing_credits_mutations=9 expected_negative_files=678')
runner_path.write_text(runner)
for _, _, path, pin in rows:
    (root / 'pins' / pin).write_text(hashlib.sha256((root / path).read_bytes()).hexdigest() + '\n')
transcript_line, = [line for line in runner.splitlines() if line.startswith('transcript=')]
transcript = shlex.split(transcript_line)[0].partition('=')[2] + '\n'
(root / 'pins/TRANSCRIPT_SHA256').write_text(hashlib.sha256(transcript.encode()).hexdigest() + '\n')

spec = importlib.util.spec_from_file_location('r72_quality', quality_path)
quality = importlib.util.module_from_spec(spec)
spec.loader.exec_module(quality)
source = quality_path.read_text()
source = source.replace('EXPECTED_RUNTIME_NEGATIVE_COUNT = 669', 'EXPECTED_RUNTIME_NEGATIVE_COUNT = 678')
authority, names, _, _ = quality.preseal_authority_assignments(runner)
source = source.replace('EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENT_COUNT = 1470',
                        f'EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENT_COUNT = {len(names)}')
for name, digest in {
    'EXPECTED_PRESEAL_AUTHORITY_ASSIGNMENTS_SHA256': hashlib.sha256(authority.encode()).hexdigest(),
    'EXPECTED_RUNNER_SHA256': hashlib.sha256(runner.encode()).hexdigest(),
}.items():
    source, count = re.subn(r'(' + name + r' = \(\n    ")[0-9a-f]{64}("\n\))',
                          lambda m: m[1] + digest + m[2], source)
    assert count == 1
old = quality.EXPECTED_RUNNER_FUNCTION_SHA256['check_sources']
new = hashlib.sha256(quality.runner_function_source(runner, 'check_sources').encode()).hexdigest()
assert source.count(old) == 1
source = source.replace(old, new)
quality_path.write_text(source)
(root / 'pins/NEGATIVE_QUALITY_CHECKER_SHA256').write_text(
    hashlib.sha256(quality_path.read_bytes()).hexdigest() + '\n')
print(f'Registered 1 positive, 9 negatives, {len(names)} authority assignments')
