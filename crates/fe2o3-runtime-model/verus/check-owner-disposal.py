#!/usr/bin/env python3
"""Qualify shared Unknown disposal and independently executed logical disposal.

Developer normal-return contents and conditional custody only. This lane does
not establish physical capacity, unwind safety, native disposal authority,
pending-consumer admission, or performance parity.
"""
import argparse
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
ROOT = CRATE / 'verus/context_owner_disposal_paired_v1.rs'
RAW = CRATE / 'verus/context_owner_disposal_execution_v1.rs'
BODY = CRATE / 'src/context_version_journal/disposal_bodies.rs'
MODEL = CRATE / 'verus/context_owner_disposal_model_v1.rs'
BRIDGE = CRATE / 'verus/context_owner_disposal_correspondence_v1.rs'
UNREAD_BODY = CRATE / 'src/context_read_leases/guard_bodies.rs'
PREVIOUS = CRATE / 'verus/check-owner-retirement.py'
FROZEN = 'c0f0766d3fb3c5a588dcfb1a6c1355f9efb73cb8'
FROZEN_INPUTS = Path('docs/evidence/dev-owner-retirement-2026-09-23/records/source.json')
POLICY_NEW = [CRATE / ('verus/context_owner_disposal_' + suffix + '_v1.rs')
              for suffix in ('decisions', 'bodies', 'model', 'invariant', 'correspondence', 'witnesses')]
OWNERS = [('context_version_journal', 'ContextVersionJournalV1', None),
          ('context_read_leases', 'ContextReadLeasedJournalV1', 'journal'),
          ('context_producer_reads', 'ContextProducerReadJournalV1', 'stable')]
NEW = [ROOT, RAW, BODY, *POLICY_NEW,
       *[CRATE / ('src/' + owner + '/disposal_baseline.rs') for owner, _, _ in OWNERS],
       CRATE / 'src/context_version_journal/disposal_shared_tests.rs',
       CRATE / 'src/context_producer_reads/tests/disposal_shared.rs']
PROJECTIONS = [(BODY, 'audited-disposal.rs', 8)]
LIFETIME_PROJECTIONS = {
    CRATE / 'verus/context_owner_disposal_bodies_v1.rs': 3,
}

# Measured whole-root/CPU inventories and reviewed runtime adapter pins.
# Campaigns, including probes, reject incomplete configuration.
PAIRED_VERIFIED = 1085
RAW_VERIFIED = 356
RETIREMENT_VERIFIED = 999
CPU_TEST_RESULTS = [(996, 0, 18, 0, 0), (27, 0, 0, 0, 0)]
ADAPTER_PINS = {
    'context_version_journal.rs': '977c226889296f583483a3e94578995e1309c2e8f713118c6f20ce042b1d814e',
    'context_version_journal/settlement.rs': '801b0c3953a7f089e7406867e411a784a06b4142f44379e1a97abbb770adbee6',
    'context_version_journal/settlement/disposal.rs': 'bf4a2279026304856d1dc51e52070d9d72856ca3cbc371cb183257be62be3f74',
    'context_read_leases.rs': 'b7ae17a7d8ac52c7cfc80f377e8bc33e24788b9df7c1001e11558a9537101064',
    'context_producer_reads.rs': 'c8ae01af7df3805f32e8ffafe22f20749ff0b3cdb9a7395dd57957140af4ccf5',
    'context_version_journal/guard_test_support.rs': 'd16ee92b0cd298e8a2cf703a472e660964ec86fcf5414c7aa1d0a7bd2131eef2',
    'context_read_leases/guard_test_support.rs': '4582de07b59804d4e55e72b1297dca5a0e76bab8bbb630a51a0fea472f5417c4',
    'context_producer_reads/guard_test_support.rs': 'de3e00ba1d5745ab9105760e3db99b2f84c820603abd48f041210a15400860e0',
    'context_version_journal/settlement_tests.rs': '3044393f226a49718207678a8f4abf364ee99bf82b638ed26f2b6a0396370075',
    'context_producer_reads/tests.rs': 'ccd83be4eac408ef34f4856d74568a7c1db204d7514a9ed3c160e3b19ceca383',
}
DOCUMENT_PINS = {
    Path('docs/runtime-producer-read-reservations-v1.md'): '902ad997745fcc8f0447711dbff76bcdc05d71780cbafbfa246bf9d374f3a7d2',
}


def mutation(name, macro, function, before, after):
    return (name, BODY, 'macro_rules! disposal_' + macro + '_body {',
            'production', function, before, after, 'execution')


PLAN = 'ContextVersionJournalV1::unknown_disposal_plan_observed_v1'
EXECUTE = 'ContextVersionJournalV1::dispose_unknown_observed_v1'
COMMIT = 'shared_disposal_commit_v1'
VALIDATE = 'validate_unknown_disposal_observed_v1'
MUTATIONS = [
    mutation('unknown_only', 'plan', PLAN, 'if !unknown {', 'if false {'),
    mutation('roster_length', 'plan', PLAN, '$roster.len() != $count', '$roster.len() > $count'),
    mutation('complete_chain', 'plan', PLAN,
             'if let Err(error) = $chain($journal, $writer, $head, $count) { return Err(error); }', ''),
    mutation('allocation_identity', 'plan', PLAN,
             'destination.allocation.slot != member.allocation.slot', 'false'),
    mutation('device_identity', 'plan', PLAN,
             'destination.device.local != allocation.device.local', 'false'),
    mutation('extent_identity', 'plan', PLAN,
             'destination.byte_extent != allocation.byte_extent', 'false'),
    mutation('empty_admission', 'plan', PLAN,
             'let writer_returns = match', 'if $count == 0 { return Ok(($head, $count)); } let writer_returns = match'),
    mutation('evidence_identity', 'execute', EXECUTE,
             '$evidence.writer.slot != $writer.slot || !$same_key($evidence.writer.key, $writer.key)', 'false'),
    mutation('evidence_before_header', 'execute', EXECUTE,
             'if let Err(error) = $header($journal, $writer, true) { return Err(error); }',
             'if $evidence.writer.slot != $writer.slot || !$same_key($evidence.writer.key, $writer.key) '
             '{ return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch); } '
             'if let Err(error) = $header($journal, $writer, true) { return Err(error); }'),
    mutation('writer_logical_headroom', 'plan', PLAN, 'writer_returns > $journal.writer_capacity || ', ''),
    mutation('writer_observed_headroom', 'plan', PLAN, ' || writer_returns > $writer_capacity', ''),
    mutation('member_logical_headroom', 'plan', PLAN, 'member_returns > $journal.allocation_capacity || ', ''),
    mutation('member_observed_headroom', 'plan', PLAN, ' || member_returns > $member_capacity', ''),
    mutation('allocation_logical_headroom', 'plan', PLAN, 'allocation_returns > $journal.allocation_capacity || ', ''),
    mutation('allocation_observed_headroom', 'plan', PLAN, ' || allocation_returns > $allocation_capacity', ''),
    mutation('unchecked_return', 'plan', PLAN,
             'let allocation_returns = match $journal.allocation_free.len().checked_add($count) {\n'
             '                Some(value) => value, None => return Err(ContextVersionJournalErrorV1::InvalidState),\n'
             '            };',
             'let allocation_returns = $journal.allocation_free.len() + $count;'),
    mutation('scratch_bound', 'plan', PLAN, ' || $count > $journal.scratch.len()', ''),
    mutation('scratch_busy', 'scratch_scan', 'shared_disposal_scratch_scan_v1',
             '$journal.scratch[$index].is_some()', 'false'),
    mutation('stage_plan', 'stage', 'shared_disposal_stage_v1',
             'prior_lineage: member.prior_lineage', 'prior_lineage: 0'),
    mutation('scratch_clear', 'commit', COMMIT, '.take()', ''),
    mutation('allocation_remove', 'commit', COMMIT, '$journal.allocations[plan.allocation.slot] = None;', ''),
    mutation('allocation_return', 'commit', COMMIT, '$journal.allocation_free.push(plan.allocation.slot);', ''),
    mutation('allocation_order', 'commit', COMMIT,
             '$journal.allocation_free.push(plan.allocation.slot);', '$journal.allocation_free.push($index);'),
    mutation('member_remove', 'commit', COMMIT, '$journal.members[plan.member_slot] = None;', ''),
    mutation('member_return', 'commit', COMMIT, '$journal.member_free.push(plan.member_slot);', ''),
    mutation('writer_remove', 'commit', COMMIT, '$journal.writers[$writer.slot] = None;', ''),
    mutation('writer_return', 'commit', COMMIT, '$journal.free.push($writer.slot);', ''),
    mutation('journal_frame', 'commit', COMMIT,
             '$journal.free.push($writer.slot);', '$journal.free.push($writer.slot); $journal.registration_watermark = 0;'),
]
for prefix, owner in [('stable', 'ContextReadLeasedJournalV1'), ('producer', 'ContextProducerReadJournalV1')]:
    MUTATIONS.extend([
        (prefix + '_busy', UNREAD_BODY, 'macro_rules! unread_writes_body {', 'production',
         owner + '::require_disposal_unread_writes_v1', 'count != 0', 'count > 1', 'execution'),
        mutation(prefix + '_guard', 'owner_validate', owner + '::' + VALIDATE,
                 'if let Err(error) = $owner.$unread($roster) { return Err(error); }', ''),
        mutation(prefix + '_frame', 'owner_execute', owner + '::dispose_unknown_observed_v1',
                 '$owner.$field.$execute($writer, $evidence $($observations)*)',
                 '{ $owner.next_incarnation = 0; $owner.$field.$execute($writer, $evidence $($observations)*) }'),
    ])
MUTATIONS.extend([
    ('skip_actual', BRIDGE, 'fn owner_disposal_paired_exec_v1(', 'production', 'owner_disposal_paired_exec_v1',
     'let result = actual.dispose_unknown_observed_v1(writer, &ContextWriterDisposalEvidenceV1 { writer: evidence, allocations: roster },\n'
     '        writer_storage, member_storage, allocation_storage);',
     'let result: Result<(), ReadErrorV1> = Err(ReadErrorV1::AllocationBusy);', 'execution'),
    ('skip_model', BRIDGE, 'fn owner_disposal_paired_exec_v1(', 'production', 'owner_disposal_paired_exec_v1',
     'let model_result = logical::disposal_producer_exec_v1(model, model_writer, model_evidence, model_roster,\n'
     '        writer_storage, member_storage, allocation_storage);',
     'let model_result: Result<(), logical::ReadErrorV1> = Err(logical::ReadErrorV1::AllocationBusy);', 'execution'),
    ('model_allocation_remove', MODEL, 'pub fn disposal_commit_exec_v1(', 'owner_disposal_model',
     'disposal_commit_exec_v1', 'journal.allocations[plan.allocation.slot] = None;', '', 'execution'),
])


def need(condition, message):
    if not condition:
        raise ValueError(message)


def module(repo, path, name):
    spec = importlib.util.spec_from_file_location(name, repo / path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def configuration():
    for name, value in [('PAIRED_VERIFIED', PAIRED_VERIFIED), ('RAW_VERIFIED', RAW_VERIFIED),
                        ('RETIREMENT_VERIFIED', RETIREMENT_VERIFIED)]:
        need(type(value) is int and value > 0, 'final measured count required: ' + name)
    need(type(CPU_TEST_RESULTS) is list and len(CPU_TEST_RESULTS) == 2
         and all(type(row) is tuple and len(row) == 5 and all(type(n) is int and n >= 0 for n in row)
                 for row in CPU_TEST_RESULTS), 'final unit/doctest inventory required')
    need(all(type(pin) is str and re.fullmatch(r'[0-9a-f]{64}', pin) for pin in ADAPTER_PINS.values()),
         'final reviewed runtime adapter pins required')
    need(all(type(pin) is str and re.fullmatch(r'[0-9a-f]{64}', pin) for pin in DOCUMENT_PINS.values()),
         'final reviewed document pins required')


def method(source, name):
    anchors = list(re.finditer(r'^    (?:pub(?:\([^)]*\))? )?fn ' + re.escape(name) + r'\(', source, re.M))
    need(len(anchors) == 1, 'unique frozen method: ' + name)
    start = anchors[0].start()
    return source[start:source.index('\n    }\n', start) + len('\n    }\n')]


def frozen_baseline(previous, owner, ty, field):
    names = ['validate_unknown_disposal', 'dispose_unknown'] + ([] if field else ['unknown_disposal_plan'])
    pieces = []
    for name in names:
        body = re.sub(r'^    (?:pub(?:\([^)]*\))? )?fn ', '    pub(crate) fn ', method(previous, name))
        body = re.sub(r'\b(validate_unknown_disposal|dispose_unknown|unknown_disposal_plan)\b',
                      lambda match: 'baseline_' + match[0] + '_v1', body)
        if field:
            body = body.replace('self.' + field + '.baseline_validate_unknown_disposal_v1(',
                                'self.' + field + '\n            .baseline_validate_unknown_disposal_v1(')
        else:
            body = body.replace('self.baseline_unknown_disposal_plan_v1(writer, canonical).map(|_| ())',
                                'self.baseline_unknown_disposal_plan_v1(writer, canonical)\n            .map(|_| ())')
            body = body.replace('let (mut head, count) = self.baseline_unknown_disposal_plan_v1(writer, evidence.allocations)?;',
                                'let (mut head, count) =\n            self.baseline_unknown_disposal_plan_v1(writer, evidence.allocations)?;')
        pieces.append(body.rstrip())
    return ('// Frozen at ' + FROZEN + '; only method names and visibility are redirected.\n'
            'use super::*;\n\nimpl ' + ty + ' {\n' + '\n\n'.join(pieces) + '\n}\n')


def independent_model(source, policy):
    code = policy.code_only(source)
    need(re.fullmatch(r'\s*use\s+super\s*::\s*\*\s*;\s*verus\s*!\s*\{.*\}\s*', code, re.S) is not None,
         'exact independent model import/envelope')
    need(len(re.findall(r'\buse\b', code)) == 1, 'no independent model import aliases')
    macros = re.findall(r'\b([A-Za-z_][A-Za-z0-9_]*)\s*!\s*[({\[]', code)
    need(macros == ['verus'], 'no shared macro invocation in independent model')
    need(re.search(r'\bproduction\b|\bshared_[A-Za-z0-9_]+\b|\b[A-Za-z_][A-Za-z0-9_]*_body\b'
                   r'|\bmacro_export\b|\bmacro_use\b', code) is None,
         'independent model cannot reference production or shared bodies')


def source_roster(repo, inherited):
    expected = {p for p in inherited | set(NEW) if p.is_relative_to(CRATE / 'src')}
    paths = list((repo / CRATE / 'src').rglob('*'))
    need(not any(p.is_symlink() for p in paths), 'no runtime source symlinks')
    need({p.relative_to(repo) for p in paths if p.is_file()} == expected, 'closed runtime source roster')
    need({p.name for p in (repo / CRATE).iterdir()} == {'Cargo.toml', 'src', 'verus'}, 'closed Cargo discovery roster')


def scan_leaf(stage, path, policy):
    target = stage / path
    if path in LIFETIME_PROJECTIONS:
        data = target.read_text()
        need(data.count("<'_>") == LIFETIME_PROJECTIONS[path], 'exact anonymous-lifetime projection')
        target = stage / ('audited-lifetimes-' + path.name)
        target.write_text(data.replace("<'_>", '<>'))
    policy.scan(target)


def authenticate(repo, inherited, scalar, git_repo=None):
    def old(path):
        return subprocess.check_output(['git', '-C', str(git_repo or repo), 'show', FROZEN + ':' + str(path)])

    changed = {CRATE / 'src' / path for path in ADAPTER_PINS}
    need(changed <= inherited, 'reviewed adapter paths inherited')
    need(set(DOCUMENT_PINS) <= inherited and not changed.intersection(DOCUMENT_PINS),
         'separate reviewed document paths inherited')
    for path in inherited:
        current = (repo / path).read_bytes()
        if path in changed:
            pin = ADAPTER_PINS[str(path.relative_to(CRATE / 'src'))]
            need(type(pin) is str and scalar.digest(current) == pin, 'exact reviewed adapter: ' + str(path))
        elif path in DOCUMENT_PINS:
            need(scalar.digest(current) == DOCUMENT_PINS[path], 'exact reviewed document: ' + str(path))
        else:
            need(current == old(path), 'unchanged inherited input: ' + str(path))
    for owner, ty, field in OWNERS:
        source = CRATE / ('src/' + owner + '.rs') if field else CRATE / 'src/context_version_journal/settlement/disposal.rs'
        expected = frozen_baseline(old(source).decode(), owner, ty, field)
        need((repo / CRATE / ('src/' + owner + '/disposal_baseline.rs')).read_text() == expected,
             'independent frozen methods: ' + owner)
    retirement_root = CRATE / 'verus/context_owner_retirement_paired_v1.rs'
    expected = old(retirement_root).decode().replace('mod production {',
        '#[path = "context_owner_disposal_model_v1.rs"]\nmod owner_disposal_model;\nuse owner_disposal_model::*;\n\n'
        '#[path = "context_owner_disposal_invariant_v1.rs"]\nmod owner_disposal_invariant;\nuse owner_disposal_invariant::*;\n\nmod production {')
    expected = expected.replace('include!("context_owner_retirement_execution_v1.rs");',
                                'include!("context_owner_disposal_execution_v1.rs");')
    expected = expected.replace('    include!("context_owner_retirement_witnesses_v1.rs");',
        '    include!("context_owner_retirement_witnesses_v1.rs");\n'
        '    include!("context_owner_disposal_correspondence_v1.rs");\n'
        '    include!("context_owner_disposal_witnesses_v1.rs");')
    need((repo / ROOT).read_text() == expected, 'exact disposal paired root')
    expected = ('// Extend the actual owner declarations; physical capacity remains an observation.\n'
                'include!("context_owner_retirement_execution_v1.rs");\n'
                'include!("../src/context_version_journal/disposal_bodies.rs");\n'
                'include!("context_owner_disposal_decisions_v1.rs");\n'
                'include!("context_owner_disposal_bodies_v1.rs");\n')
    need((repo / RAW).read_text() == expected, 'exact disposal actual root')
    policy = module(repo, Path('examples/wave64_collectives_v1/check-proof-source.py'), 'disposal_independence_policy')
    independent_model((repo / MODEL).read_text(), policy)
    source_roster(repo, inherited)


def check_negative(scalar, name, status, observed):
    need(type(status) is int and status == 1, 'normal negative exit')
    scalar.negative_report(observed['result'])
    errors = [row for row in observed['diagnostics'] if row['level'] == 'error']
    allowed = {'assertion failed', 'postcondition not satisfied', 'precondition not satisfied',
               'invariant not satisfied before loop', 'invariant not satisfied at end of loop body'}
    if name == 'unchecked_return':
        allowed.add('possible arithmetic underflow/overflow')
    need(any(row['message'] in allowed for row in errors)
         and all(row['message'] in allowed or re.fullmatch(r'aborting due to \d+ previous errors?', row['message'])
                 for row in errors), 'logical failure, not syntax, timeout, or solver resource exhaustion')


def recorder_selftest():
    import copy
    repo = Path(__file__).resolve().parents[3]
    retirement = module(repo, PREVIOUS, 'disposal_selftest_retirement')
    retirement.recorder_selftest()
    scalar = module(repo, retirement.PREVIOUS, 'disposal_selftest_scalar')
    policy = module(repo, Path('examples/wave64_collectives_v1/check-proof-source.py'), 'disposal_selftest_policy')
    valid = {'result': {'encountered-error': True, 'encountered-vir-error': False, 'verified': 0,
                        'errors': 1, 'is-verifying-entire-crate': False},
             'diagnostics': [{'level': 'error', 'message': 'postcondition not satisfied'}]}
    check_negative(scalar, 'allocation_remove', 1, valid)
    initial_invariant = copy.deepcopy(valid)
    initial_invariant['diagnostics'][0]['message'] = 'invariant not satisfied before loop'
    check_negative(scalar, 'complete_chain', 1, initial_invariant)
    arithmetic = copy.deepcopy(valid)
    arithmetic['diagnostics'][0]['message'] = 'possible arithmetic underflow/overflow'
    check_negative(scalar, 'unchecked_return', 1, arithmetic)
    rejected = 0
    for fault in ('wrong_arithmetic', 'resource', 'frontend', 'abort_only', 'status', 'bool_status', 'frontend_flag', 'count'):
        row, status = copy.deepcopy(valid), 1
        if fault in ('wrong_arithmetic', 'resource', 'frontend', 'abort_only'):
            row['diagnostics'][0]['message'] = {
                'wrong_arithmetic': 'possible arithmetic underflow/overflow',
                'resource': 'function body check: Resource limit (rlimit) exceeded',
                'frontend': 'unexpected token', 'abort_only': 'aborting due to 1 previous error',
            }[fault]
        elif fault == 'status':
            status = 0
        elif fault == 'bool_status':
            status = True
        elif fault == 'frontend_flag':
            row['result']['encountered-vir-error'] = True
        else:
            row['result']['errors'] = 0
        try:
            check_negative(scalar, 'allocation_remove', status, row)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted invalid disposal negative: ' + fault)
    prefix, suffix = 'use super::*;\nverus! {\n', '\n}\n'
    independent_model(prefix + 'pub fn sample() { if !false { assert(true); } }' + suffix, policy)
    independent_model('// production::shared_disposal_body!()\n' + prefix + 'pub fn sample() {}' + suffix, policy)
    for body in ('disposal_commit_body!();', 'production::run();', 'shared_disposal_commit_v1();',
                 'use super::production as other;', 'alias!{}', 'disposal_commit_body ();'):
        try:
            independent_model(prefix + 'pub fn sample() { ' + body + ' }' + suffix, policy)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted coupled independent model: ' + body)
    for owner, ty, field in OWNERS:
        source = CRATE / ('src/' + owner + '.rs') if field else CRATE / 'src/context_version_journal/settlement/disposal.rs'
        previous = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(source)], text=True)
        expected = frozen_baseline(previous, owner, ty, field)
        need((repo / CRATE / ('src/' + owner + '/disposal_baseline.rs')).read_text() == expected,
             'baseline selftest: ' + owner)
    legacy = module(repo, Path('docs/evidence/dev-producer-stable-2026-09-22/check.py'), 'disposal_selftest_mutations')
    need(len({row[0] for row in MUTATIONS}) == len(MUTATIONS), 'unique mutation names')
    for row in MUTATIONS:
        legacy.mutated((repo / row[1]).read_bytes(), row)
    independent_model((repo / MODEL).read_text(), policy)
    with tempfile.TemporaryDirectory(prefix='disposal-policy-selftest-') as temporary:
        stage = Path(temporary)
        for path in POLICY_NEW:
            target = stage / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((repo / path).read_bytes())
            scan_leaf(stage, path, policy)
        for path, projection, count in PROJECTIONS:
            data = (repo / path).read_text()
            need(data.count('macro_rules!') == count, 'selftest macro roster')
            target = stage / projection
            target.write_text(data.replace('macro_rules!', ''))
            policy.scan(target)
    base = module(repo, legacy.BASE, 'disposal_selftest_json')
    frozen_inputs = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(FROZEN_INPUTS)], text=True)
    inherited = {Path(p) for p in base.unique_json(frozen_inputs)['inputs']} | {FROZEN_INPUTS}
    pins = ADAPTER_PINS.copy()
    try:
        # This is only a selftest fixture. Main qualification never fills pins.
        for path, pin in pins.items():
            if pin is None:
                ADAPTER_PINS[path] = scalar.digest((repo / CRATE / 'src' / path).read_bytes())
        with tempfile.TemporaryDirectory(prefix='disposal-source-selftest-') as temporary:
            stage = Path(temporary)
            for path in sorted(inherited | set(NEW)):
                target = stage / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((repo / path).read_bytes())
            authenticate(stage, inherited, scalar, git_repo=repo)
            baseline = CRATE / 'src/context_version_journal/disposal_baseline.rs'
            adapter = CRATE / 'src/context_version_journal/settlement/disposal.rs'
            extra = CRATE / 'src/unregistered_disposal_source.rs'
            registration = ('#[path = "context_owner_disposal_model_v1.rs"]\n'
                            'mod owner_disposal_model;\nuse owner_disposal_model::*;\n\n').encode()
            controls = [
                ('baseline_drift', baseline, b'if !unknown {', b'if false {'),
                ('adapter_drift', adapter, None, b'\n'),
                ('document_drift', Path('docs/runtime-producer-read-reservations-v1.md'), None, b'\n'),
                ('model_registration', ROOT, registration, b''),
                ('unregistered_source', extra, None, b'// Unregistered source fixture.\n'),
            ]
            for name, path, before, after in controls:
                target = stage / path
                original = target.read_bytes() if target.exists() else None
                try:
                    if before is not None:
                        need(original is not None and original.count(before) == 1, 'unique source-auth mutation: ' + name)
                        changed = original.replace(before, after, 1)
                    else:
                        changed = (original or b'') + after
                    target.write_bytes(changed)
                    try:
                        authenticate(stage, inherited, scalar, git_repo=repo)
                    except ValueError:
                        rejected += 1
                    else:
                        raise ValueError('accepted source-auth mutation: ' + name)
                finally:
                    if original is None:
                        target.unlink()
                    else:
                        target.write_bytes(original)
            authenticate(stage, inherited, scalar, git_repo=repo)
    finally:
        ADAPTER_PINS.clear()
        ADAPTER_PINS.update(pins)
    print(f'PASS: disposal diagnostic, independence, and source-auth checks; {rejected} malformed cases rejected; '
          f'{len(MUTATIONS)} scoped mutation anchors authenticated')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', type=Path, required=True)
    parser.add_argument('--probe', action='store_true')
    parser.add_argument('--resume', action='store_true')
    parser.add_argument('--stop-after')
    args = parser.parse_args()
    configuration()
    repo, output, verus = args.repo.resolve(), args.output.resolve(), args.verus.resolve(strict=True)
    os.chdir(repo)
    retirement = module(repo, PREVIOUS, 'disposal_retirement_check')
    scalar = module(repo, retirement.PREVIOUS, 'disposal_scalar_check')
    lock = scalar.campaign_lock(output)
    history = module(repo, scalar.PREVIOUS, 'disposal_history_check')
    settlement = module(repo, history.PREVIOUS, 'disposal_settlement_check')
    writer = module(repo, settlement.PREVIOUS, 'disposal_writer_check')
    previous = module(repo, writer.PREVIOUS, 'disposal_enrollment_check')
    ancestor = module(repo, previous.PREVIOUS, 'disposal_unknown_check')
    legacy = module(repo, settlement.LEGACY, 'disposal_legacy_check')
    base = module(repo, legacy.BASE, 'disposal_process_recorder')
    policy = module(repo, legacy.POLICY, 'disposal_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    frozen_inputs = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(FROZEN_INPUTS)], text=True)
    inherited = {Path(p) for p in base.unique_json(frozen_inputs)['inputs']} | {FROZEN_INPUTS}
    authenticate(repo, inherited, scalar)
    sources = sorted(inherited | set(NEW))
    inputs = set(sources) | {Path(__file__).resolve().relative_to(repo)}
    before = {str(p): scalar.digest((repo / p).read_bytes()) for p in sorted(inputs)}
    commit = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
    if not args.probe:
        for path, expected in before.items():
            data = subprocess.check_output(['git', '-C', str(repo), 'show', commit + ':' + path])
            need(scalar.digest(data) == expected, 'source commit mismatch: ' + path)
    for path, pin in legacy.PINS.items():
        need(scalar.digest((repo / path).read_bytes()) == pin, 'source pin: ' + str(path))
    source = {'commit': commit, 'probe': args.probe, 'inputs': before}
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup',
           'VERUS_Z3_PATH': str(verus.parent / 'z3'), 'CARGO_TARGET_DIR': str(args.target.resolve()),
           'TMPDIR': str(output)}
    cases = [('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None),
             ('raw-regression', None), ('retirement-regression', None)]
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
    cpu_parser = module(repo, settlement.CPU_PARSER, 'disposal_cpu_parser').test_results

    def proof_root(name):
        return RAW if name == 'raw-regression' else retirement.ROOT if name == 'retirement-regression' else ROOT

    def validate(name, row, stdout, stderr):
        if name in ('closure-before', 'closure-after'):
            need(row['command'] == closure_command and row['status'] == 0 and not stderr and stdout ==
                 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'pinned Verus distribution')
        elif name in cpu_commands:
            need(row['command'] == cpu_commands[name] and row['status'] == 0, 'CPU check: ' + name)
            if name == 'test':
                need(cpu_parser(stdout) == CPU_TEST_RESULTS, 'exact CPU test inventory')
            if name == 'recorder-test':
                need(stdout == 'PASS: receipt and checkpoint replay checks; 29 malformed or conflicting operations rejected\n'
                     'PASS: retirement negative diagnostic policy; 8 malformed cases rejected\n'
                     'PASS: disposal diagnostic, independence, and source-auth checks; 19 malformed cases rejected; '
                     f'{len(MUTATIONS)} scoped mutation anchors authenticated\n' and not stderr, 'recorder regression inventory')
        else:
            root, change = proof_root(name), mutations[name]
            recorded_root = Path(row['command'][-1])
            need(recorded_root.is_absolute() and str(recorded_root).endswith('/' + str(root)), 'resumed proof root')
            stage = Path(str(recorded_root)[:-len(str(root)) - 1])
            need(stage.parent == output and stage.name.startswith('sources-'), 'owned recorded source stage')
            need(row['command'] == legacy.command(verus, stage / root, change), 'exact proof command')
            observed = legacy.normalized(base, stdout, stderr, stage)
            scalar.check_verifier(observed['verus'])
            if change:
                check_negative(scalar, name, row['status'], observed)
            else:
                count = RAW_VERIFIED if name == 'raw-regression' else RETIREMENT_VERIFIED if name == 'retirement-regression' else PAIRED_VERIFIED
                need(row['status'] == 0 and not observed['diagnostics'], 'clean terminal positive')
                need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                     'success': True, 'verified': count, 'errors': 0, 'is-verifying-entire-crate': True}), 'exact whole-root positive')
            return observed

    campaign = scalar.Campaign(output, source, names, base, legacy.same, validate, args.resume)

    def source_unchanged(label):
        source_roster(repo, inherited)
        need(before == {str(p): scalar.digest((repo / p).read_bytes()) for p in sorted(inputs)}, label)

    def stop(name):
        if args.stop_after != name or name == 'release-build':
            return False
        source_unchanged('checkpoint source drift')
        print('CHECKPOINT: ' + name + ' checked; campaign incomplete', flush=True)
        return True

    for phase in ['before', 'after']:
        campaign.run('closure-' + phase, closure_command, 120, env)
        if stop('closure-' + phase):
            return
        if phase == 'after':
            break
        for name, change in cases:
            if name in campaign.rows:
                print(name, campaign.proofs[name]['result'], '(reused)', flush=True)
                if stop(name):
                    return
                continue
            with tempfile.TemporaryDirectory(prefix='sources-', dir=output) as temporary:
                stage = Path(temporary)
                for path in sources:
                    target = stage / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    data = (repo / path).read_bytes()
                    need(scalar.digest(data) == before[str(path)], 'staging source drift: ' + str(path))
                    target.write_bytes(legacy.mutated(data, change) if change and path == change[1] else data)
                policies = (set(legacy.POLICY_SOURCES) | (set(ancestor.NEW) - {ancestor.ROOT, ancestor.RAW, ancestor.BODY})
                            | set(previous.POLICY_NEW) | set(writer.POLICY_NEW) | set(settlement.POLICY_NEW)
                            | set(history.POLICY_NEW) | set(scalar.POLICY_NEW) | set(retirement.POLICY_NEW) | set(POLICY_NEW))
                for path in policies:
                    scan_leaf(stage, path, policy)
                independent_model((stage / MODEL).read_text(), policy)
                for path, projection, count in [*legacy.PROJECTIONS, (ancestor.BODY, 'audited-unknown-wrappers.rs', 3),
                                                *previous.PROJECTIONS, *writer.PROJECTIONS, *settlement.PROJECTIONS,
                                                *scalar.PROJECTIONS, *retirement.PROJECTIONS, *PROJECTIONS]:
                    data = (stage / path).read_text()
                    need(data.count('macro_rules!') == count, 'macro roster')
                    stripped = stage / projection
                    stripped.write_text(data.replace('macro_rules!', ''))
                    policy.scan(stripped)
                campaign.run(name, legacy.command(verus, stage / proof_root(name), change), 610, env)
                print(name, campaign.proofs[name]['result'], flush=True)
            if stop(name):
                return
        need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']), 'matching whole-root positives')
    for name, command in cpu:
        campaign.run(name, command, 600, env)
        print(name, 'passed', flush=True)
        if stop(name):
            return
    source_unchanged('source drift')
    (output / 'inputs-after.json').write_text(json.dumps(before, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(campaign.proofs, indent=2) + '\n')


if __name__ == '__main__':
    if sys.argv[1:] == ['--selftest']:
        recorder_selftest()
    else:
        main()
