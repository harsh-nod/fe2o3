#!/usr/bin/env python3
"""Record scalar public enrollment, independent model correspondence and custody.

Not constructor, physical-allocation, native-admission, or performance qualification.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
CRATE = Path('crates/fe2o3-runtime-model')
ROOT = CRATE / 'verus/context_owner_scalar_enrollment_paired_v1.rs'
RAW = CRATE / 'verus/context_owner_scalar_enrollment_execution_v1.rs'
BODY = CRATE / 'src/context_version_journal/scalar_enrollment_bodies.rs'
BRIDGE = CRATE / 'verus/context_owner_scalar_enrollment_correspondence_v1.rs'
PREVIOUS = CRATE / 'verus/check-owner-settlement-historical.py'
FROZEN = '376a60343c906313fee87d1c78744bda662cb248'
FROZEN_INPUTS = Path('docs/evidence/dev-owner-settlement-history-2026-09-22/records/source.json')
OWNERS = [('context_version_journal', 'ContextVersionJournalV1', None),
          ('context_read_leases', 'ContextReadLeasedJournalV1', 'journal'),
          ('context_producer_reads', 'ContextProducerReadJournalV1', 'stable')]
POLICY_NEW = [CRATE / ('verus/context_owner_scalar_enrollment_' + suffix + '_v1.rs')
              for suffix in ('bodies', 'model', 'correspondence', 'witnesses')]
NEW = [ROOT, RAW, BODY, *POLICY_NEW,
       *[CRATE / ('src/' + owner + '/scalar_enrollment_baseline.rs') for owner, _, _ in OWNERS],
       CRATE / 'src/context_version_journal/scalar_enrollment_tests.rs',
       CRATE / 'src/context_producer_reads/tests/scalar_enrollment_shared.rs']
PROJECTIONS = [(BODY, 'audited-scalar-enrollment.rs', 2)]
PAIRED_VERIFIED = 929
MUTATIONS = [
    ('header_order', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'if $key.context_generation != $journal.context_generation',
     'if $key.context_generation != $journal.context_generation || $device.context_generation != $journal.context_generation', 'execution'),
    ('replay_order', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'let mut $index = 0usize;', 'if $journal.allocation_free.len() == 0 { return Err(ContextVersionJournalErrorV1::AllocationCapacity); } let mut $index = 0usize;', 'execution'),
    ('replay_bound', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'while $index < $journal.allocations.len()', 'while $index < $journal.allocations.len() && $index < $journal.allocation_capacity', 'execution'),
    ('replay_context', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'entry.key.context_generation == $key.context_generation && ', '', 'execution'),
    ('raw_capacity', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'let mut $index = 0usize;', 'if $journal.allocation_free.len() > $journal.allocation_capacity { return Err(ContextVersionJournalErrorV1::InvalidState); } let mut $index = 0usize;', 'execution'),
    ('tail', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'let slot = $journal.allocation_free[$journal.allocation_free.len() - 1];',
     'let slot = $journal.allocation_free[0];', 'execution'),
    ('vacancy', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     ' || $journal.allocations[slot].is_some()', '', 'execution'),
    ('pop', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'let _ = $journal.allocation_free.pop();', '', 'execution'),
    ('extent', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'byte_extent: $extent', 'byte_extent: 0', 'execution'),
    ('epoch', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'attempt_epoch: 0', 'attempt_epoch: 1', 'execution'),
    ('return_key', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'Ok(ContextAllocationReferenceV1 { slot, key: $key })',
     'Ok(ContextAllocationReferenceV1 { slot, key: ContextAllocationKeyV1 { context_generation: $key.context_generation, local: 0 } })', 'execution'),
    ('journal_frame', BODY, 'macro_rules! scalar_enrollment_body {', 'production', 'ContextVersionJournalV1::enroll_allocation',
     'let _ = $journal.allocation_free.pop();',
     'let _ = $journal.allocation_free.pop(); $journal.registration_watermark = 0;', 'execution'),
    ('stable_frame', BODY, 'macro_rules! scalar_enrollment_forward_body {', 'production', 'ContextReadLeasedJournalV1::enroll_allocation',
     '$owner.$field.enroll_allocation($key, $device, $extent)',
     '{ $owner.next_incarnation = 0; $owner.$field.enroll_allocation($key, $device, $extent) }', 'execution'),
    ('producer_frame', BODY, 'macro_rules! scalar_enrollment_forward_body {', 'production', 'ContextProducerReadJournalV1::enroll_allocation',
     '$owner.$field.enroll_allocation($key, $device, $extent)',
     '{ $owner.next_incarnation = 0; $owner.$field.enroll_allocation($key, $device, $extent) }', 'execution'),
    ('stable_skip', BODY, 'macro_rules! scalar_enrollment_forward_body {', 'production', 'ContextReadLeasedJournalV1::enroll_allocation',
     '$owner.$field.enroll_allocation($key, $device, $extent)', 'Err(ContextVersionJournalErrorV1::AllocationCapacity)', 'execution'),
    ('producer_skip', BODY, 'macro_rules! scalar_enrollment_forward_body {', 'production', 'ContextProducerReadJournalV1::enroll_allocation',
     '$owner.$field.enroll_allocation($key, $device, $extent)', 'Err(ContextVersionJournalErrorV1::AllocationCapacity)', 'execution'),
    ('skip_actual', BRIDGE, 'fn owner_scalar_enrollment_paired_exec_v1(', 'production', 'owner_scalar_enrollment_paired_exec_v1',
     'let result = actual.enroll_allocation(entry.key, entry.device, entry.byte_extent);',
     'let result: Result<AllocationReferenceV1, ReadErrorV1> = Err(ReadErrorV1::AllocationCapacity);', 'execution'),
    ('skip_model', BRIDGE, 'fn owner_scalar_enrollment_paired_exec_v1(', 'production', 'owner_scalar_enrollment_paired_exec_v1',
     'let model_result = logical::scalar_enrollment_exec_v1(&mut model.stable.journal, model_entry);',
     'let model_result: Result<logical::AllocationReferenceV1, logical::EnrollmentErrorV1> = Err(logical::EnrollmentErrorV1::AllocationCapacity);', 'execution'),
]


def need(condition, message):
    if not condition:
        raise ValueError(message)


def module(repo, path, name):
    spec = importlib.util.spec_from_file_location(name, repo / path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def negative_report(result):
    need(set(result) == {'encountered-error', 'encountered-vir-error', 'verified', 'errors',
                         'is-verifying-entire-crate'}, 'closed negative result schema')
    need(result['encountered-error'] is True and result['encountered-vir-error'] is False
         and result['is-verifying-entire-crate'] is False, 'typed negative flags')
    need(type(result['verified']) is int and result['verified'] >= 0
         and type(result['errors']) is int and result['errors'] > 0, 'typed negative counts')


def source_roster(repo, inherited):
    expected = {p for p in inherited | set(NEW) if p.is_relative_to(CRATE / 'src')}
    paths = list((repo / CRATE / 'src').rglob('*'))
    need(not any(p.is_symlink() for p in paths), 'no runtime source symlinks')
    need({p.relative_to(repo) for p in paths if p.is_file()} == expected, 'closed runtime source roster')
    need({p.name for p in (repo / CRATE).iterdir()} == {'Cargo.toml', 'src', 'verus'}, 'closed Cargo discovery roster')


def authenticate(repo, inherited, history, git_repo=None):
    def old(path):
        return subprocess.check_output(['git', '-C', str(git_repo or repo), 'show', FROZEN + ':' + str(path)]).decode()
    def method(source):
        start = source.index('    pub fn enroll_allocation(')
        return source[start:source.index('\n    }\n', start) + len('\n    }\n')]
    changed = set()
    for owner, ty, field in OWNERS:
        path = CRATE / ('src/' + owner + '.rs')
        previous, current = old(path), (repo / path).read_text()
        original, candidate = method(previous), method(current)
        baseline = original.replace('pub fn enroll_allocation(', 'pub(crate) fn baseline_enroll_allocation_v1(')
        if field:
            baseline = baseline.replace('self.' + field + '.enroll_allocation(',
                                        'self.' + field + '\n            .baseline_enroll_allocation_v1(')
            adapter = 'scalar_enrollment_forward_body!(self, ' + field + ', key, device, extent)'
            anchor = 'mod settlement_templates {\n    include!("context_version_journal/settlement_wrapper_bodies.rs");\n}'
        else:
            adapter = ('scalar_enrollment_body!(writer_rust_expr, self, key, device, byte_extent, '
                       'issuable_context_id, Self::count_indexed_access, index, [])')
            anchor = 'mod writer_lifecycle_templates {\n    include!("context_version_journal/writer_lifecycle_bodies.rs");\n}'
        expected = ('// Frozen at ' + FROZEN + '; only names and visibility are redirected.\n'
                    'use super::*;\n\nimpl ' + ty + ' {\n' + baseline + '}\n')
        frozen = CRATE / ('src/' + owner + '/scalar_enrollment_baseline.rs')
        need((repo / frozen).read_text() == expected, 'exact closed frozen method: ' + owner)
        expected_method = original.split(' {', 1)[0] + ' {\n        ' + adapter + '\n    }\n'
        need(re.sub(r'\s+', '', candidate) == re.sub(r'\s+', '', expected_method), 'exact scalar adapter: ' + owner)
        addition = ('\n\n#[allow(unused_macros)]\n#[macro_use]\nmod scalar_enrollment_templates {\n'
                    '    include!("context_version_journal/scalar_enrollment_bodies.rs");\n}\n\n'
                    '#[cfg(test)]\nmod scalar_enrollment_baseline;')
        need(previous.count(anchor) == 1, 'unique runtime module anchor')
        expected = previous.replace(original, candidate).replace(anchor, anchor + addition)
        need(current == expected, 'only authenticated owner deltas: ' + owner)
        changed.add(path)
    for path, anchor, addition in [
        (CRATE / 'src/context_version_journal/tests.rs', 'mod writer_lifecycle;',
         '\n\n#[path = "scalar_enrollment_tests.rs"]\nmod scalar_enrollment;'),
        (CRATE / 'src/context_producer_reads/tests.rs', 'mod release_shared;', '\nmod scalar_enrollment_shared;'),
    ]:
        previous = old(path)
        need(previous.count(anchor) == 1, 'unique test module anchor')
        need((repo / path).read_text() == previous.replace(anchor, anchor + addition), 'test registration only')
        changed.add(path)
    for path in inherited - changed:
        need((repo / path).read_bytes() == old(path).encode(), 'unchanged inherited input: ' + str(path))
    source_roster(repo, inherited)
    expected = old(history.ROOT).replace('mod production {',
        '#[path = "context_owner_scalar_enrollment_model_v1.rs"]\n'
        'mod owner_scalar_enrollment_model;\nuse owner_scalar_enrollment_model::*;\n\nmod production {')
    expected = expected.replace('include!("context_owner_settlement_execution_v1.rs");',
                                'include!("context_owner_scalar_enrollment_execution_v1.rs");')
    expected = expected.replace('    include!("context_owner_settlement_historical_witnesses_v1.rs");',
        '    include!("context_owner_settlement_historical_witnesses_v1.rs");\n'
        '    include!("context_owner_scalar_enrollment_correspondence_v1.rs");\n'
        '    include!("context_owner_scalar_enrollment_witnesses_v1.rs");')
    need((repo / ROOT).read_text() == expected, 'exact paired root additions')
    expected = ('// Extend the actual owner universe without replacing its declarations.\n'
                'include!("context_owner_settlement_execution_v1.rs");\n'
                'include!("../src/context_version_journal/scalar_enrollment_bodies.rs");\n'
                'include!("context_owner_scalar_enrollment_bodies_v1.rs");\n')
    need((repo / RAW).read_text() == expected, 'exact scalar actual root')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', type=Path, required=True)
    parser.add_argument('--probe', action='store_true')
    args = parser.parse_args()
    repo, output, verus = args.repo.resolve(), args.output.resolve(), args.verus.resolve(strict=True)
    os.chdir(repo)
    history = module(repo, PREVIOUS, 'scalar_history_check')
    settlement = module(repo, history.PREVIOUS, 'scalar_settlement_check')
    writer = module(repo, settlement.PREVIOUS, 'historical_settlement_writer_check')
    previous = module(repo, writer.PREVIOUS, 'historical_settlement_enrollment_check')
    ancestor = module(repo, previous.PREVIOUS, 'historical_settlement_unknown_check')
    legacy = module(repo, settlement.LEGACY, 'historical_settlement_legacy_check')
    base = module(repo, legacy.BASE, 'historical_settlement_process_recorder')
    policy = module(repo, legacy.POLICY, 'historical_settlement_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    inherited = {Path(p) for p in json.loads((repo / FROZEN_INPUTS).read_text())['inputs']}
    inherited.add(FROZEN_INPUTS)
    authenticate(repo, inherited, history)
    sources = sorted(inherited | set(NEW))
    inputs = set(sources) | {Path(__file__).resolve().relative_to(repo)}
    before = {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}
    commit = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
    if not args.probe:
        for path, expected in before.items():
            data = subprocess.check_output(['git', '-C', str(repo), 'show', commit + ':' + path])
            need(digest(data) == expected, 'source commit mismatch: ' + path)
    for path, pin in legacy.PINS.items():
        need(digest((repo / path).read_bytes()) == pin, 'source pin: ' + str(path))
    output.mkdir()
    (output / 'source.json').write_text(json.dumps({'commit': commit, 'probe': args.probe, 'inputs': before}, indent=2) + '\n')
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup',
           'VERUS_Z3_PATH': str(verus.parent / 'z3'), 'CARGO_TARGET_DIR': str(args.target.resolve())}
    results = {}
    for phase in ['before', 'after']:
        status, _, _ = base.run_owned(['/bin/sh', str(repo / legacy.CLOSURE), str(verus.parent), str(repo / legacy.MANIFEST)],
                                      120, output / ('closure-' + phase), env)
        need(status == 0, 'pinned Verus distribution')
        if phase == 'after':
            break
        cases = [('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None), ('raw-regression', None), ('settlement-regression', None)]
        for name, mutation in cases:
            with tempfile.TemporaryDirectory(prefix='sources-', dir=output) as temporary:
                stage = Path(temporary)
                for path in sources:
                    target = stage / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    data = (repo / path).read_bytes()
                    need(digest(data) == before[str(path)], 'staging source drift: ' + str(path))
                    target.write_bytes(legacy.mutated(data, mutation) if mutation and path == mutation[1] else data)
                for path in set(legacy.POLICY_SOURCES) | (set(ancestor.NEW) - {ancestor.ROOT, ancestor.RAW, ancestor.BODY}) | set(previous.POLICY_NEW) | set(writer.POLICY_NEW) | set(settlement.POLICY_NEW) | set(history.POLICY_NEW) | set(POLICY_NEW):
                    policy.scan(stage / path)
                for path, projection, count in [*legacy.PROJECTIONS, (ancestor.BODY, 'audited-unknown-wrappers.rs', 3), *previous.PROJECTIONS, *writer.PROJECTIONS, *settlement.PROJECTIONS, *PROJECTIONS]:
                    data = (stage / path).read_text()
                    need(data.count('macro_rules!') == count, 'macro roster')
                    stripped = stage / projection
                    stripped.write_text(data.replace('macro_rules!', ''))
                    policy.scan(stripped)
                command = legacy.command(verus, stage / (RAW if name == 'raw-regression' else history.ROOT if name == 'settlement-regression' else ROOT), mutation)
                status, stdout, stderr = base.run_owned(command, 610, output / name, env)
                observed = legacy.normalized(base, stdout, stderr, stage)
                if mutation:
                    # Reuse the scoped logical-failure checks; this developer record
                    # captures diagnostics, rather than claiming precommitted replay.
                    negative_report(observed['result'])
                    legacy.check_result(base, status, stdout, stderr, stage, mutation, {name: observed})
                else:
                    need(status == 0 and not observed['diagnostics'], 'clean terminal positive')
                    report = observed['result']
                    need(legacy.same(report, {'encountered-error': False, 'encountered-vir-error': False,
                                    'success': True, 'verified': 310 if name == 'raw-regression' else 916 if name == 'settlement-regression' else PAIRED_VERIFIED,
                                    'errors': 0, 'is-verifying-entire-crate': True}), 'exact whole-root positive')
                results[name] = observed
                print(name, observed['result'], flush=True)
        need(legacy.same(results['positive-before'], results['positive-after']), 'matching whole-root positives')
    for name, command in [
        ('compiler', ['rustc', '+1.97.1', '-Vv']),
        ('format', ['cargo', '+1.97.1', 'fmt', '--check', '-p', 'fe2o3-runtime-model']),
        ('test', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model']),
        ('clippy', ['cargo', '+1.97.1', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']),
        ('release-build', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--no-run']),
    ]:
        status, stdout, _ = base.run_owned(command, 600, output / name, env)
        need(status == 0, 'CPU check: ' + name)
        if name == 'test':
            parse = module(repo, settlement.CPU_PARSER, 'settlement_cpu_parser').test_results
            need(parse(stdout) == [(966, 0, 18, 0, 0), (27, 0, 0, 0, 0)], 'exact CPU test inventory')
        print(name, 'passed', flush=True)
    after = {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}
    source_roster(repo, inherited)
    need(before == after, 'source drift')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
