#!/usr/bin/env python3
"""Record scalar public enrollment, independent model correspondence and custody.

Not constructor, physical-allocation, native-admission, or performance qualification.
"""
import argparse
import fcntl
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
VERIFIER_IDENTITY = {'profile': 'release', 'version': '0.2026.08.09.92f466f',
                     'platform': {'os': 'linux', 'arch': 'x86_64'},
                     'toolchain': '1.97.1-x86_64-unknown-linux-gnu',
                     'commit': '92f466f247f45128c630d1c843fd6e27d2115587'}
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


def check_verifier(identity):
    need(identity == VERIFIER_IDENTITY, 'exact pinned verifier identity')


def negative_report(result):
    need(set(result) == {'encountered-error', 'encountered-vir-error', 'verified', 'errors',
                         'is-verifying-entire-crate'}, 'closed negative result schema')
    need(result['encountered-error'] is True and result['encountered-vir-error'] is False
         and result['is-verifying-entire-crate'] is False, 'typed negative flags')
    need(type(result['verified']) is int and result['verified'] >= 0
         and type(result['errors']) is int and result['errors'] > 0, 'typed negative counts')


def completed_record(folder, parse, command=None):
    need(folder.is_dir() and not folder.is_symlink(), 'normal case directory')
    need({p.name for p in folder.iterdir()} == {'record.json', 'stdout.log', 'stderr.log'},
         'incomplete or unexpected existing case; inspect it before retrying')
    need(all(p.is_file() and not p.is_symlink() for p in folder.iterdir()), 'normal case files')
    row = parse((folder / 'record.json').read_text())
    need(set(row) == {'command', 'started_ns', 'finished_ns', 'process_group', 'status', 'group_absent'},
         'interrupted or malformed receipt cannot be reused')
    need(type(row['status']) is int and row['status'] in (0, 1) and row['group_absent'] is True,
         'normal terminal receipt required')
    need(all(type(row[k]) is int and row[k] > 0 for k in ('started_ns', 'finished_ns', 'process_group'))
         and row['started_ns'] <= row['finished_ns'], 'receipt integers/order')
    need(isinstance(row['command'], list) and row['command']
         and all(isinstance(token, str) and token for token in row['command']), 'command tokens')
    need(command is None or row['command'] == command, 'exact resumed command')
    return row, (folder / 'stdout.log').read_text(), (folder / 'stderr.log').read_text()


def campaign_lock(output):
    lock = (output.parent / (output.name + '.lock')).open('a')
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as error:
        lock.close()
        raise ValueError('another controller owns this campaign') from error
    return lock


def record_hashes(folder):
    return {name: digest((folder / name).read_bytes())
            for name in ('record.json', 'stdout.log', 'stderr.log')}


class Campaign:
    """Only controller-accepted, semantically checked prefixes are reusable."""

    def __init__(self, output, source, names, base, same, validate, resume):
        self.output, self.names, self.base = output, names, base
        self.validate, self.rows, self.proofs, self.accepted = validate, {}, {}, []
        self.last_finished = 0
        if not resume:
            output.mkdir()
            (output / 'source.json').write_text(json.dumps(source, indent=2) + '\n')
            self.save_acceptance()
            return
        need(output.is_dir() and not output.is_symlink(), 'normal resume directory')
        need(all(p.is_file() and not p.is_symlink() for p in
                 (output / 'source.json', output / 'accepted.json')), 'normal controller files')
        need(same(base.unique_json((output / 'source.json').read_text()), source), 'exact resume source identity')
        self.accepted = base.unique_json((output / 'accepted.json').read_text())
        need(type(self.accepted) is list and len(self.accepted) <= len(names), 'accepted prefix list')
        for index, entry in enumerate(self.accepted):
            need(type(entry) is dict and set(entry) == {'name', 'files'}
                 and entry['name'] == names[index], 'strict accepted prefix')
        expected = {'source.json', 'accepted.json'} | set(names[:len(self.accepted)])
        # Interrupted finalization may leave either summary, but never permits more execution.
        if len(self.accepted) == len(names):
            expected |= {p.name for p in output.iterdir() if p.name in ('inputs-after.json', 'results.json')}
        need({p.name for p in output.iterdir()} == expected, 'unaccepted or missing campaign records')
        for entry in self.accepted:
            name = entry['name']
            row, stdout, stderr = completed_record(output / name, base.unique_json)
            need(same(entry['files'], record_hashes(output / name)), 'accepted record hashes')
            self.check(name, row, stdout, stderr)
        for name, value in (('inputs-after.json', source['inputs']), ('results.json', self.results())):
            path = output / name
            if path.exists() or path.is_symlink():
                need(path.is_file() and not path.is_symlink()
                     and same(base.unique_json(path.read_text()), value), 'existing final summary')

    def check(self, name, row, stdout, stderr):
        need(self.last_finished <= row['started_ns'], 'serial receipt order')
        result = self.validate(name, row, stdout, stderr)
        if result is not None:
            self.proofs[name] = result
        self.last_finished = row['finished_ns']
        self.rows[name] = (row, stdout, stderr)

    def results(self):
        return self.proofs

    def save_acceptance(self):
        temporary = self.output / 'accepted.json.tmp'
        temporary.write_text(json.dumps(self.accepted, indent=2) + '\n')
        temporary.replace(self.output / 'accepted.json')

    def run(self, name, command, timeout, env):
        if name in self.rows:
            row, stdout, stderr = self.rows[name]
            need(row['command'] == command, 'exact resumed command')
            return row['status'], stdout, stderr
        need(len(self.accepted) < len(self.names) and name == self.names[len(self.accepted)], 'next prefix command')
        folder = self.output / name
        # A normal-looking receipt is insufficient: run_owned can reject retained
        # descendants after recording their cleanup. Never mark that call accepted.
        self.base.run_owned(command, timeout, folder, env)
        row, stdout, stderr = completed_record(folder, self.base.unique_json, command)
        self.check(name, row, stdout, stderr)
        self.accepted.append({'name': name, 'files': record_hashes(folder)})
        self.save_acceptance()
        return row['status'], stdout, stderr


def recorder_selftest():
    import copy
    base = module(Path(__file__).resolve().parents[3], CRATE / 'verus/check-journal-issuance.py', 'scalar_recorder_test_base')
    with tempfile.TemporaryDirectory(prefix='fe2o3-scalar-recorder-test-') as temporary:
        folder = Path(temporary)
        (folder / 'stdout.log').write_text('stdout\n')
        (folder / 'stderr.log').write_text('')
        valid = {'command': ['tool', '--check'], 'started_ns': 10, 'finished_ns': 20,
                 'process_group': 42, 'status': 0, 'group_absent': True}
        for status in (0, 1):
            row = dict(valid, status=status)
            (folder / 'record.json').write_text(json.dumps(row))
            observed, stdout, stderr = completed_record(folder, base.unique_json, valid['command'])
            need(observed == row and stdout == 'stdout\n' and stderr == '', 'valid receipt round trip')
        faults = [('exception', 'CampaignInterrupted'), ('status', -9), ('status', True),
                  ('group_absent', 1), ('group_absent', False), ('process_group', 0),
                  ('started_ns', True), ('finished_ns', 1), ('command', ['different']), ('command', [1])]
        for key, value in faults:
            row = copy.deepcopy(valid)
            row[key] = value
            (folder / 'record.json').write_text(json.dumps(row))
            try:
                completed_record(folder, base.unique_json, valid['command'])
            except ValueError:
                continue
            raise ValueError('accepted malformed resume receipt: ' + key)
        (folder / 'record.json').unlink()
        try:
            completed_record(folder, base.unique_json, valid['command'])
        except ValueError:
            pass
        else:
            raise ValueError('accepted incomplete case')
        (folder / 'record.json').write_text(json.dumps(valid)[:-1] + ', "status": 0}')
        try:
            completed_record(folder, base.unique_json, valid['command'])
        except ValueError:
            pass
        else:
            raise ValueError('accepted duplicate receipt key')
    rejected = 12

    def rejects(action):
        nonlocal rejected
        try:
            action()
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted malformed campaign')

    source = {'commit': 'test', 'probe': True, 'inputs': {'source': 'hash'}}
    names = ['first', 'second', 'third']
    calls = []

    def fake_run(command, timeout, folder, env):
        calls.append(folder.name)
        folder.mkdir()
        start = 10 * (names.index(folder.name) + 1)
        row = dict(valid, command=command, started_ns=start, finished_ns=start + 5)
        (folder / 'record.json').write_text(json.dumps(row))
        stdout = json.dumps(VERIFIER_IDENTITY) + '\n'
        (folder / 'stdout.log').write_text(stdout)
        (folder / 'stderr.log').write_text('')
        return 0, stdout, ''

    def validate(name, row, stdout, stderr):
        check_verifier(base.unique_json(stdout))
        need(row['command'] == ['tool', name] and row['status'] == 0
             and stderr == '', 'checked fake command')

    class FakeBase:
        unique_json = staticmethod(base.unique_json)
        run_owned = staticmethod(fake_run)

    def controller(folder, resume=True, identity=source):
        return Campaign(folder, identity, names, FakeBase, lambda a, b: json.dumps(a, sort_keys=True)
                        == json.dumps(b, sort_keys=True), validate, resume)

    with tempfile.TemporaryDirectory(prefix='fe2o3-scalar-campaign-test-') as temporary:
        root = Path(temporary)
        folder = root / 'checkpoints'
        for count, name in enumerate(names):
            campaign = controller(folder, resume=count > 0)
            for previous in names[:count]:
                campaign.run(previous, ['tool', previous], 1, {})
            campaign.run(name, ['tool', name], 1, {})
        need(calls == names, 'checkpoint continuation executes each command once')
        snapshot = {str(p.relative_to(folder)): p.read_bytes() for p in folder.rglob('*') if p.is_file()}
        replay = controller(folder)
        for name in names:
            replay.run(name, ['tool', name], 1, {})
        need(calls == names and snapshot == {str(p.relative_to(folder)): p.read_bytes()
             for p in folder.rglob('*') if p.is_file()}, 'complete replay does not execute or rewrite records')
        rejects(lambda: controller(folder, identity=dict(source, probe=False)))
        rejects(lambda: replay.run('first', ['wrong'], 1, {}))
        with campaign_lock(folder):
            rejects(lambda: campaign_lock(folder))
        with campaign_lock(folder):
            pass

        import shutil
        for fault in ('hole', 'future', 'missing_marker', 'hash', 'later_semantics', 'later_metadata', 'order',
                      'reordered_marker', 'extra_stage', 'final_summary', 'summary_symlink'):
            changed = root / fault
            shutil.copytree(folder, changed)
            if fault in ('hole', 'future'):
                shutil.rmtree(changed / 'first')
                if fault == 'future':
                    (changed / 'accepted.json').write_text('[]\n')
            elif fault == 'missing_marker':
                (changed / 'accepted.json').unlink()
            elif fault in ('hash', 'later_semantics', 'later_metadata'):
                content = json.dumps(dict(VERIFIER_IDENTITY, profile='other')) if fault == 'later_metadata' else 'bad\n'
                (changed / 'third/stdout.log').write_text(content)
                if fault != 'hash':
                    entries = base.unique_json((changed / 'accepted.json').read_text())
                    entries[-1]['files'] = record_hashes(changed / 'third')
                    (changed / 'accepted.json').write_text(json.dumps(entries))
            elif fault == 'order':
                path = changed / 'third/record.json'
                row = base.unique_json(path.read_text())
                row['started_ns'] = 1
                path.write_text(json.dumps(row))
                entries = base.unique_json((changed / 'accepted.json').read_text())
                entries[-1]['files'] = record_hashes(changed / 'third')
                (changed / 'accepted.json').write_text(json.dumps(entries))
            elif fault == 'reordered_marker':
                entries = base.unique_json((changed / 'accepted.json').read_text())
                entries.reverse()
                (changed / 'accepted.json').write_text(json.dumps(entries))
            elif fault == 'extra_stage':
                (changed / 'sources-interrupted').mkdir()
            elif fault == 'final_summary':
                (changed / 'results.json').write_text('{"extra": true}\n')
            else:
                (changed / 'results.json').symlink_to(root / 'missing')
            rejects(lambda: controller(changed))
            need(calls == names, 'invalid prefix rejected before execution')

        # Reproduce run_owned's post-receipt rejection without creating an orphan.
        def rejected_cleanup(command, timeout, folder, env):
            fake_run(command, timeout, folder, env)
            raise ValueError('command left process-group members')

        changed = root / 'rejected_cleanup'
        campaign = controller(changed, resume=False)
        FakeBase.run_owned = staticmethod(rejected_cleanup)
        rejects(lambda: campaign.run('first', ['tool', 'first'], 1, {}))
        need(base.unique_json((changed / 'accepted.json').read_text()) == [], 'rejected cleanup not accepted')
        rejects(lambda: controller(changed))
        FakeBase.run_owned = staticmethod(fake_run)
        changed = root / 'out_of_order'
        campaign = controller(changed, resume=False)
        rejects(lambda: campaign.run('second', ['tool', 'second'], 1, {}))
        need(calls == names + ['first'], 'rejected resume never reruns command')
    print(f'PASS: receipt and checkpoint replay checks; {rejected} malformed or conflicting operations rejected')


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
    parser.add_argument('--resume', action='store_true', help='Reuse only fully validated terminal records for this exact source.')
    parser.add_argument('--stop-after', help='Return at a named checked command; an incomplete campaign is not qualification.')
    args = parser.parse_args()
    repo, output, verus = args.repo.resolve(), args.output.resolve(), args.verus.resolve(strict=True)
    os.chdir(repo)
    lock = campaign_lock(output)
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
    source = {'commit': commit, 'probe': args.probe, 'inputs': before}
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup',
           'VERUS_Z3_PATH': str(verus.parent / 'z3'), 'CARGO_TARGET_DIR': str(args.target.resolve())}
    cases = [('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None), ('raw-regression', None), ('settlement-regression', None)]
    cpu = [
        ('recorder-test', ['python3', '-B', str(Path(__file__).resolve()), '--selftest']),
        ('compiler', ['rustc', '+1.97.1', '-Vv']),
        ('format', ['cargo', '+1.97.1', 'fmt', '--check', '-p', 'fe2o3-runtime-model']),
        ('test', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model']),
        ('clippy', ['cargo', '+1.97.1', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']),
        ('release-build', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--no-run']),
    ]
    names = ['closure-before', *[name for name, _ in cases], 'closure-after', *[name for name, _ in cpu]]
    need(args.stop_after is None or args.stop_after in names, 'known checkpoint name')
    closure_command = ['/bin/sh', str(repo / legacy.CLOSURE), str(verus.parent), str(repo / legacy.MANIFEST)]
    cpu_commands, mutations = dict(cpu), dict(cases)
    cpu_parser = module(repo, settlement.CPU_PARSER, 'settlement_cpu_parser').test_results

    def proof_stage(name, row):
        root = RAW if name == 'raw-regression' else history.ROOT if name == 'settlement-regression' else ROOT
        recorded_root = Path(row['command'][-1])
        need(recorded_root.is_absolute() and str(recorded_root).endswith('/' + str(root)), 'resumed proof root')
        stage = Path(str(recorded_root)[:-len(str(root)) - 1])
        need(stage.parent == output and stage.name.startswith('sources-'), 'owned recorded source stage')
        need(row['command'] == legacy.command(verus, stage / root, mutations[name]), 'exact proof command')
        return stage

    def validate(name, row, stdout, stderr):
        if name in ('closure-before', 'closure-after'):
            need(row['command'] == closure_command and row['status'] == 0 and not stderr and stdout ==
                 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'pinned Verus distribution')
        elif name in cpu_commands:
            need(row['command'] == cpu_commands[name] and row['status'] == 0, 'CPU check: ' + name)
            if name == 'test':
                need(cpu_parser(stdout) == [(966, 0, 18, 0, 0), (27, 0, 0, 0, 0)], 'exact CPU test inventory')
            if name == 'recorder-test':
                need(stdout == 'PASS: receipt and checkpoint replay checks; 29 malformed or conflicting operations rejected\n'
                     and not stderr, 'recorder regression inventory')
        else:
            stage, mutation = proof_stage(name, row), mutations[name]
            observed = legacy.normalized(base, stdout, stderr, stage)
            check_verifier(observed['verus'])
            if mutation:
                negative_report(observed['result'])
                legacy.check_result(base, row['status'], stdout, stderr, stage, mutation, {name: observed})
            else:
                need(row['status'] == 0 and not observed['diagnostics'], 'clean terminal positive')
                need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                     'success': True, 'verified': 310 if name == 'raw-regression' else 916 if name == 'settlement-regression' else PAIRED_VERIFIED,
                     'errors': 0, 'is-verifying-entire-crate': True}), 'exact whole-root positive')
            return observed

    # Preflight every cached command, result, hash and timestamp before any new work.
    campaign = Campaign(output, source, names, base, legacy.same, validate, args.resume)
    results = campaign.results()

    def run(name, command, timeout):
        return campaign.run(name, command, timeout, env)

    def stop(name):
        if args.stop_after != name or name == 'release-build':
            return False
        source_roster(repo, inherited)
        need(before == {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}, 'checkpoint source drift')
        state = 'commands complete' if len(campaign.accepted) == len(names) else 'campaign incomplete'
        print('CHECKPOINT: ' + name + ' checked; ' + state, flush=True)
        return True

    for phase in ['before', 'after']:
        run('closure-' + phase, closure_command, 120)
        if stop('closure-' + phase):
            return
        if phase == 'after':
            break
        for name, mutation in cases:
            root = RAW if name == 'raw-regression' else history.ROOT if name == 'settlement-regression' else ROOT
            if name in campaign.rows:
                print(name, results[name]['result'], '(reused)', flush=True)
                if stop(name):
                    return
                continue
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
                command = legacy.command(verus, stage / root, mutation)
                run(name, command, 610)
                print(name, results[name]['result'], flush=True)
                if stop(name):
                    return
        need(legacy.same(results['positive-before'], results['positive-after']), 'matching whole-root positives')
    for name, command in cpu:
        run(name, command, 600)
        print(name, 'passed', flush=True)
        if stop(name):
            return
    after = {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}
    source_roster(repo, inherited)
    need(before == after, 'source drift')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    if sys.argv[1:] == ['--selftest']:
        recorder_selftest()
    else:
        main()
