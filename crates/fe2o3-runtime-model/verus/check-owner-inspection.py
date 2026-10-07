#!/usr/bin/env python3
"""Qualify shared immutable getters and actual Deref implementations.

Developer normal-content inspection only. Physical addresses, allocation,
unwind, native admission and general lifecycle reachability remain separate.
"""
import argparse
import copy
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
ROOT = CRATE / 'verus/context_owner_inspection_paired_v1.rs'
RAW = CRATE / 'verus/context_owner_inspection_execution_v1.rs'
EXECUTION = CRATE / 'verus/context_owner_inspection_bodies_v1.rs'
BODY = CRATE / 'src/context_version_journal/inspection_bodies.rs'
MODEL = CRATE / 'verus/context_owner_inspection_model_v1.rs'
BRIDGE = CRATE / 'verus/context_owner_inspection_correspondence_v1.rs'
WITNESSES = CRATE / 'verus/context_owner_inspection_witnesses_v1.rs'
GUARDS = CRATE / 'verus/context_reader_guards_execution_v1.rs'
ACQUIRE = CRATE / 'verus/context_stable_acquire_bodies_v1.rs'
QUERY = CRATE / 'verus/context_producer_query_bodies_v1.rs'
DISPOSAL_WITNESSES = CRATE / 'verus/context_owner_disposal_witnesses_v1.rs'
RELEASE_WITNESSES = CRATE / 'verus/context_stable_release_witnesses_v1.rs'
PREVIOUS = CRATE / 'verus/check-owner-constructor.py'
FROZEN = '36e5c16494da7bee7d743c3c98a52d4130396f96'
FROZEN_INPUTS = Path('docs/evidence/dev-owner-constructor-2026-09-23/records/source.json')
OWNERS = [('context_version_journal', 'ContextVersionJournalV1'),
          ('context_read_leases', 'ContextReadLeasedJournalV1'),
          ('context_producer_reads', 'ContextProducerReadJournalV1')]
POLICY_NEW = [EXECUTION, MODEL, BRIDGE, WITNESSES]
NEW = [ROOT, RAW, BODY, *POLICY_NEW,
       *[CRATE / ('src/' + owner + '/inspection_baseline.rs') for owner, _ in OWNERS],
       *[CRATE / ('src/' + owner + '/tests/inspection_shared.rs') for owner, _ in OWNERS]]
MACROS = ['owner_inspection_' + name + '_body' for name in ('scalar', 'length', 'deref')]
PROJECTIONS = [(BODY, 'audited-inspection.rs', 3)]

# Missing measurements fail closed for both qualification and probe campaigns.
PAIRED_VERIFIED = 1172
RAW_VERIFIED = 384
CONSTRUCTOR_VERIFIED = 1149
CPU_TEST_RESULTS = [(1021, 0, 18, 0, 0), (27, 0, 0, 0, 0)]
WHOLE_TIMEOUT_SECONDS = 900
SCOPED_TIMEOUT_SECONDS = 600
ADAPTER_PINS = {
    'context_version_journal.rs': '996c8ea1220e49210af8962d729b92f0987dc1863b4026264bbee19df38741ce',
    'context_read_leases.rs': 'b9f77ec7e1250cf7871808b8cac43e8e0647232c4eb1cad39fa3ceeb75a2c156',
    'context_producer_reads.rs': 'fd7bf8b80e119521a5cbcbeb864af087ef8feca2176f49190e43d82e9a4ce176',
    'context_version_journal/allocation_lifecycle.rs': '1fc60051a8141fa183597edcd827d2b33f409e1ee06ae6f37b8f6411f505925c',
    'context_version_journal/tests.rs': '2d776e920e2436925b2ca982e203507a14c6f5f214b623e0b78fad9e3798eb25',
    'context_read_leases/tests.rs': '8cc0d5a0bdedf90fe536572d5b8232271f5fca6456dc2261b843de51954f65b3',
    'context_producer_reads/tests.rs': '44919ff8013c2dc05ac21b4bd0081055ae45db515d9dffa940a28aa7e3748181',
}
DOCUMENT_PINS = {
    Path('docs/runtime-producer-read-reservations-v1.md'): '8382d19dc9a23e6399e723275c8bd454d89745f2593f078b0ab5980fc8ab9f3a',
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def module(repo, path, name):
    spec = importlib.util.spec_from_file_location(name, repo / path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


CONSTRUCTOR = module(Path(__file__).resolve().parents[3], PREVIOUS, 'inspection_constructor')
check_negative = CONSTRUCTOR.check_negative
independent_model = CONSTRUCTOR.independent_model


def dependencies(repo):
    result = CONSTRUCTOR.dependencies(repo)
    result['constructor'] = CONSTRUCTOR
    return result


GETTERS = [
    ('context_generation', 'scalar', 'ContextVersionJournalV1'),
    ('allocation_capacity', 'scalar', 'ContextVersionJournalV1'),
    ('writer_capacity', 'scalar', 'ContextVersionJournalV1'),
    ('registration_watermark', 'scalar', 'ContextVersionJournalV1'),
    ('remaining_writer_slots', 'length', 'ContextVersionJournalV1'),
    ('reserved_writer_count', 'scalar', 'ContextVersionJournalV1'),
    ('remaining_allocation_slots', 'length', 'ContextVersionJournalV1'),
    ('remaining_read_slots', 'length', 'ContextReadLeasedJournalV1'),
]
MUTATIONS = [
    ('zero_' + name, BODY, 'macro_rules! owner_inspection_' + kind + '_body {',
     'production', ty + '::' + name, '$owner.$field' + ('.len()' if kind == 'length' else ''), '0', 'execution')
    for name, kind, ty in GETTERS
]
for owner, ty in [('stable', 'ContextReadLeasedJournalV1'), ('producer', 'ContextProducerReadJournalV1')]:
    for mode in ('explicit', 'auto'):
        MUTATIONS.append((owner + '_' + mode + '_trait_contract', EXECUTION,
            'impl core::ops::Deref for ' + ty + ' {', 'production',
            'inspection_' + owner + '_' + mode + '_exec_v1',
            'ensures *result == inspection_' + owner + '_projection_v1(*self),',
            'ensures true,', 'contract'))
ZERO_JOURNAL = '(0u64, 0usize, 0usize, 0u64, 0usize, 0usize, 0usize)'
for suffix in ('journal', 'stable_explicit', 'stable_auto', 'producer_explicit', 'producer_auto'):
    function = 'inspection_' + suffix + '_paired_exec_v1'
    owner = suffix.split('_')[0]
    ty = 'JournalInspectionV1' if owner == 'journal' else 'OwnerInspectionV1'
    zero = ZERO_JOURNAL if owner == 'journal' else '(' + ZERO_JOURNAL + ', 0usize)'
    MUTATIONS.extend([
        (suffix + '_skip_actual', BRIDGE, 'fn ' + function + '(', 'production', function,
         'let result = inspection_' + suffix + '_exec_v1(actual);',
         'let result: ' + ty + ' = ' + zero + ';', 'execution'),
        (suffix + '_skip_model', BRIDGE, 'fn ' + function + '(', 'production', function,
         'let model_result = logical::inspection_' + owner + '_model_exec_v1(model);',
         'let model_result: logical::' + ty + ' = ' + zero + ';', 'execution'),
    ])
for owner in ('actual', 'model'):
    MUTATIONS.append((owner + '_identity', BRIDGE, 'fn inspection_producer_auto_paired_exec_v1(',
        'production', 'inspection_producer_auto_paired_exec_v1',
        '    (result, model_result)',
        '    ' + owner + '.next_incarnation = 0;\n    (result, model_result)', 'execution'))
MUTATIONS.append(('independent_getter', MODEL, 'pub fn inspection_context_generation_model_exec_v1(',
    'owner_inspection_model', 'inspection_context_generation_model_exec_v1',
    '{ owner.context_generation }', '{ 0 }', 'execution'))


def configuration():
    for name, value in [('PAIRED_VERIFIED', PAIRED_VERIFIED), ('RAW_VERIFIED', RAW_VERIFIED),
                        ('CONSTRUCTOR_VERIFIED', CONSTRUCTOR_VERIFIED)]:
        need(type(value) is int and value > 0, 'final measured count required: ' + name)
    need(type(CPU_TEST_RESULTS) is list and len(CPU_TEST_RESULTS) == 2
         and all(type(row) is tuple and len(row) == 5 and all(type(n) is int and n >= 0 for n in row)
                 for row in CPU_TEST_RESULTS), 'final unit/doctest inventory required')
    for pins in (ADAPTER_PINS, DOCUMENT_PINS):
        need(all(type(pin) is str and re.fullmatch(r'[0-9a-f]{64}', pin) for pin in pins.values()),
             'final reviewed source pins required')
    need((WHOLE_TIMEOUT_SECONDS, SCOPED_TIMEOUT_SECONDS) == (900, 600), 'fixed inspection wall-time budgets')


def command(legacy, verus, root, change):
    result = legacy.command(verus, root, change)
    need(type(result) is list and len(result) > 5
         and result[:5] == ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '600'],
         'exact inherited timeout prefix')
    result[4] = str(WHOLE_TIMEOUT_SECONDS if change is None else SCOPED_TIMEOUT_SECONDS)
    return result


def proof_timeout(change):
    return (WHOLE_TIMEOUT_SECONDS if change is None else SCOPED_TIMEOUT_SECONDS) + 10


def replace_once(source, before, after):
    need(source.count(before) == 1, 'unique frozen inspection redirect: ' + before)
    return source.replace(before, after, 1)


def method(source, name):
    anchors = list(re.finditer(r'^    (?:pub )?(?:const )?fn ' + re.escape(name) + r'\(', source, re.M))
    need(len(anchors) == 1, 'unique frozen method: ' + name)
    start = anchors[0].start()
    return source[start:source.index('\n    }\n', start) + len('\n    }')]


def frozen_baseline(source, allocation, owner, ty):
    names = [name for name, _, actual in GETTERS if actual == ty]
    bodies = []
    for name in names:
        body = method(allocation if name == 'remaining_allocation_slots' else source, name)
        body = replace_once(body, '    pub ', '    pub(crate) ')
        bodies.append(replace_once(body, 'fn ' + name + '(', 'fn baseline_inspection_' + name + '_v1('))
    if owner != 'context_version_journal':
        target = 'ContextVersionJournalV1' if owner == 'context_read_leases' else 'ContextReadLeasedJournalV1'
        body = replace_once(method(source, 'deref'), '    fn deref(', '    pub(crate) fn baseline_inspection_deref_v1(')
        bodies.append(replace_once(body, '&Self::Target', '&' + target))
    description = {'context_version_journal': 'method names and visibility only.',
                   'context_read_leases': 'method names, visibility and Target only.',
                   'context_producer_reads': 'method name, visibility and Target only.'}[owner]
    return ('// Frozen at ' + FROZEN + '; ' + description + '\nuse super::*;\n\nimpl ' + ty
            + ' {\n' + '\n\n'.join(bodies) + '\n}\n')


def inherited_transform(path, original):
    if path == GUARDS:
        return replace_once(original, 'include!("../src/context_producer_reads/declarations.rs");',
            'include!("../src/context_producer_reads/declarations.rs");\n'
            'include!("../src/context_version_journal/inspection_bodies.rs");\n'
            'include!("context_owner_inspection_bodies_v1.rs");')
    if path == ACQUIRE:
        return replace_once(original, 'impl ContextVersionJournalV1 {\n'
            '    fn context_generation(&self) -> (result: u64)\n'
            '        ensures result == self.context_generation,\n'
            '    { self.context_generation }\n}\n\n', '')
    if path == DISPOSAL_WITNESSES:
        helper = '''#[verifier::spinoff_prover]
fn owner_disposal_mark_fixture_unknown_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1, writer: WriterReferenceV1,
    model_writer: logical::WriterReferenceV1)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires producer_represents(*old(actual), *old(model)),
        writer_reference_view(writer) == model_writer,
        owner_settlement_pending_fixture_contents_v1(*old(actual), writer),
        logical::producer_invariant_v1(*old(model)),
    ensures results.0 == Ok(()), results.1 == Ok(()),
        producer_represents(*final(actual), *final(model)),
        logical::producer_invariant_v1(*final(model)),
        final(actual).stable.journal.writers@ == seq![Some(WriterEntryV1::Unknown {
            key: writer.key, head: Some(0usize), count: 1usize })],
        owner_unknown_frame_v1(old(actual).stable.journal, final(actual).stable.journal),
        final(actual).stable.leases == old(actual).stable.leases,
        final(actual).stable.free_reads == old(actual).stable.free_reads,
        final(actual).stable.readers == old(actual).stable.readers,
        final(actual).stable.next_incarnation == old(actual).stable.next_incarnation,
        final(actual).reservations == old(actual).reservations,
        final(actual).free == old(actual).free,
        final(actual).counts == old(actual).counts,
        final(actual).next_incarnation == old(actual).next_incarnation,
{
    proof { reveal_with_fuel(owner_retained_scan_v1, 3); }
    let marked = producer_unknown_historical_exec_v1(actual, model, writer, model_writer);
    assert(marked.0 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Unknown {
        key: writer.key, head: Some(0usize), count: 1usize })]);
    marked
}

'''
        anchor = '// The fixture starts from synthetic storage; lifecycle calls on both sides are executable.\n'
        expected = replace_once(original, anchor, helper + anchor)
        before = CONSTRUCTOR.function(original, 'owner_disposal_live_witness_v1')
        after = replace_once(before,
            '    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);',
            '    let marked = owner_disposal_mark_fixture_unknown_v1(&mut actual, &mut model, writer, model_writer);')
        after = replace_once(after, '    assert(marked.0 == Ok(()));\n',
            '    assert(marked.0 == Ok(()));\n'
            '    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Unknown {\n'
            '        key: writer.key, head: Some(0usize), count: 1usize })]);\n')
        return replace_once(expected, before, after)
    if path == RELEASE_WITNESSES:
        before = CONSTRUCTOR.function(original, 'stable_release_live_witness_v1')
        anchor = '    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };\n'
        after = replace_once(before, anchor, anchor +
            '    assert(actual.leases@ =~= seq![\n'
            '        Some(ReadLeaseV1 { reference: ContextReadLeaseReferenceV1 { slot: 0, incarnation: 1, consumer }, request }),\n'
            '        Some(ReadLeaseV1 { reference: ContextReadLeaseReferenceV1 { slot: 1, incarnation: 2, consumer },\n'
            '            request: ContextAllocationReadV1 { byte_len: 8, ..request } }),\n'
            '        Some(ReadLeaseV1 { reference: ContextReadLeaseReferenceV1 { slot: 2, incarnation: 3, consumer }, request }),\n'
            '        None,\n'
            '    ]);\n')
        return replace_once(original, before, after)
    need(path == QUERY, 'closed inherited proof transform')
    return replace_once(original,
        '        // Production\'s immutable Deref projection is source-authenticated separately.\n'
        '        producer_status_body!(&self.stable.journal, request, producer_writer_same_exec_v1)',
        '        proof { reveal(inspection_stable_projection_v1); }\n'
        '        producer_status_body!(&self.stable, request, producer_writer_same_exec_v1)')


def inherited_sources(repo, base):
    data = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(FROZEN_INPUTS)], text=True)
    return {Path(p) for p in base.unique_json(data)['inputs']} | {FROZEN_INPUTS}


def source_roster(repo, inherited):
    expected = {p for p in inherited | set(NEW) if p.is_relative_to(CRATE / 'src')}
    paths = list((repo / CRATE / 'src').rglob('*'))
    need(not any(p.is_symlink() for p in paths), 'no runtime source symlinks')
    need({p.relative_to(repo) for p in paths if p.is_file()} == expected, 'closed runtime source roster')
    need({p.name for p in (repo / CRATE).iterdir()} == {'Cargo.toml', 'src', 'verus'}, 'closed Cargo discovery roster')


def authenticate(repo, inherited, scalar, git_repo=None):
    def old(path):
        return subprocess.check_output(['git', '-C', str(git_repo or repo), 'show', FROZEN + ':' + str(path)])

    changed = {CRATE / 'src' / path for path in ADAPTER_PINS}
    proofs = {GUARDS, ACQUIRE, QUERY, DISPOSAL_WITNESSES, RELEASE_WITNESSES}
    need(len(changed) == 7 and changed <= inherited, 'seven reviewed runtime/test adapters')
    need(len(NEW) == len(set(NEW)) == 13 and not set(NEW).intersection(inherited), 'thirteen new inspection sources')
    need(proofs <= inherited and not proofs.intersection(changed | set(DOCUMENT_PINS)), 'closed inherited proof transforms')
    need(set(DOCUMENT_PINS) <= inherited and not changed.intersection(DOCUMENT_PINS), 'separate reviewed document paths')
    for path in inherited:
        current = (repo / path).read_bytes()
        if path in changed:
            need(scalar.digest(current) == ADAPTER_PINS[str(path.relative_to(CRATE / 'src'))], 'exact reviewed adapter: ' + str(path))
        elif path in DOCUMENT_PINS:
            need(scalar.digest(current) == DOCUMENT_PINS[path], 'exact reviewed document: ' + str(path))
        elif path in proofs:
            need(current == inherited_transform(path, old(path).decode()).encode(), 'exact inherited proof transform: ' + str(path))
        else:
            need(current == old(path), 'unchanged inherited input: ' + str(path))
    allocation = old(CRATE / 'src/context_version_journal/allocation_lifecycle.rs').decode()
    for owner, ty in OWNERS:
        expected = frozen_baseline(old(CRATE / ('src/' + owner + '.rs')).decode(), allocation, owner, ty)
        need((repo / CRATE / ('src/' + owner + '/inspection_baseline.rs')).read_text() == expected,
             'independent frozen inspection: ' + owner)
    expected = old(CONSTRUCTOR.ROOT).decode()
    expected = replace_once(expected, 'mod production {',
        '#[path = "context_owner_inspection_model_v1.rs"]\nmod owner_inspection_model;\nuse owner_inspection_model::*;\n\nmod production {')
    expected = replace_once(expected, 'include!("context_owner_constructor_execution_v1.rs");',
                            'include!("context_owner_inspection_execution_v1.rs");')
    expected = replace_once(expected, '    include!("context_owner_constructor_witnesses_v1.rs");',
        '    include!("context_owner_constructor_witnesses_v1.rs");\n'
        '    include!("context_owner_inspection_correspondence_v1.rs");\n'
        '    include!("context_owner_inspection_witnesses_v1.rs");')
    need((repo / ROOT).read_text() == expected, 'exact inspection paired root')
    expected = ('// Shared immutable getters and genuine Deref implementations enter at the owner declarations.\n'
                'include!("context_owner_constructor_execution_v1.rs");\n')
    need((repo / RAW).read_text() == expected, 'exact inspection actual root')
    need(re.findall(r'^macro_rules! (\w+) \{', (repo / BODY).read_text(), re.M) == MACROS, 'exact inspection macro roster')
    policy = module(repo, Path('examples/wave64_collectives_v1/check-proof-source.py'), 'inspection_independence_policy')
    independent_model((repo / MODEL).read_text(), policy)
    source_roster(repo, inherited)


def scan_sources(stage, deps):
    CONSTRUCTOR.scan_sources(stage, deps)
    for path in POLICY_NEW:
        deps['disposal'].scan_leaf(stage, path, deps['policy'])
    independent_model((stage / MODEL).read_text(), deps['policy'])
    for path, projection, count in PROJECTIONS:
        data = (stage / path).read_text()
        need(data.count('macro_rules!') == count, 'inspection macro roster')
        target = stage / projection
        target.write_text(data.replace('macro_rules!', ''))
        deps['policy'].scan(target)


def recorder_transcript():
    return ('PASS: receipt and checkpoint replay checks; 29 malformed or conflicting operations rejected\n'
            'PASS: retirement negative diagnostic policy; 8 malformed cases rejected\n'
            'PASS: inspection wall-time budgets; 3 whole-root and 25 scoped command shapes checked; '
            '5 malformed legacy prefixes rejected\n'
            f'PASS: inspection diagnostic, independence, and source-auth checks; {35 + len(DOCUMENT_PINS)} malformed cases rejected; '
            f'{len(MUTATIONS)} scoped mutation anchors authenticated\n')


def command_selftest(legacy):
    verus = Path('/verifier/verus')
    for root in (ROOT, RAW, CONSTRUCTOR.ROOT):
        expected = legacy.command(verus, Path('/stage') / root, None)
        expected[4] = '900'
        need(command(legacy, verus, Path('/stage') / root, None) == expected, 'whole-root command shape')
    need(proof_timeout(None) == 910, 'whole-root outer timeout')
    for change in MUTATIONS:
        need(command(legacy, verus, Path('/stage') / ROOT, change)
             == legacy.command(verus, Path('/stage') / ROOT, change), 'scoped command shape')
        need(proof_timeout(change) == 610, 'scoped outer timeout')

    class ChangedLegacy:
        def __init__(self, tokens):
            self.tokens = tokens

        def command(self, _verus, _root, _change):
            return self.tokens.copy()

    original = legacy.command(verus, Path('/stage') / ROOT, None)
    for index, replacement in enumerate(('/bin/timeout', '--background', '--signal=KILL', '--kill-after=10', '900')):
        changed = original.copy()
        changed[index] = replacement
        try:
            command(ChangedLegacy(changed), verus, Path('/stage') / ROOT, None)
        except ValueError as error:
            need(str(error) == 'exact inherited timeout prefix', 'timeout-prefix rejecting boundary')
        else:
            raise ValueError('accepted changed legacy timeout prefix: ' + str(index))
    print('PASS: inspection wall-time budgets; 3 whole-root and 25 scoped command shapes checked; '
          '5 malformed legacy prefixes rejected')


def recorder_selftest():
    repo = Path(__file__).resolve().parents[3]
    deps = dependencies(repo)
    deps['retirement'].recorder_selftest()
    scalar, policy, legacy, base = (deps[name] for name in ('scalar', 'policy', 'legacy', 'base'))
    command_selftest(legacy)
    valid = {'result': {'encountered-error': True, 'encountered-vir-error': False, 'verified': 0,
                        'errors': 1, 'is-verifying-entire-crate': False},
             'diagnostics': [{'level': 'error', 'message': 'postcondition not satisfied'}]}
    for message in ('assertion failed', 'postcondition not satisfied', 'precondition not satisfied',
                    'invariant not satisfied before loop', 'invariant not satisfied at end of loop body'):
        row = copy.deepcopy(valid)
        row['diagnostics'][0]['message'] = message
        check_negative(scalar, 'zero_context_generation', 1, row)
    rejected = 0
    for fault in ('arithmetic', 'resource', 'frontend', 'abort_only', 'status', 'bool_status', 'frontend_flag', 'count'):
        row, status = copy.deepcopy(valid), 1
        if fault in ('arithmetic', 'resource', 'frontend', 'abort_only'):
            row['diagnostics'][0]['message'] = {
                'arithmetic': 'possible arithmetic underflow/overflow',
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
            check_negative(scalar, 'zero_context_generation', status, row)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted invalid inspection negative: ' + fault)
    prefix, suffix = 'use super::*;\nverus! {\n', '\n}\n'
    independent_model(prefix + 'pub fn sample() { if !false { assert(true); } }' + suffix, policy)
    independent_model('// production::owner_inspection_scalar_body!()\n' + prefix + 'pub fn sample() {}' + suffix, policy)
    for body in ('owner_inspection_scalar_body!();', 'production::run();', 'shared_inspection_v1();',
                 'use super::production as other;', 'alias!{}', 'owner_inspection_scalar_body ();'):
        try:
            independent_model(prefix + 'pub fn sample() { ' + body + ' }' + suffix, policy)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted coupled independent model: ' + body)
    need(len(MUTATIONS) == len({row[0] for row in MUTATIONS}) == 25, 'exact unique mutation roster')
    need(sum(row[-1] == 'contract' for row in MUTATIONS) == 4, 'four labeled trait contract controls')
    for row in MUTATIONS:
        legacy.mutated((repo / row[1]).read_bytes(), row)
    inherited = inherited_sources(repo, base)
    with tempfile.TemporaryDirectory(prefix='inspection-source-selftest-') as temporary:
        stage = Path(temporary)
        for path in sorted(inherited | set(NEW)):
            target = stage / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((repo / path).read_bytes())
        authenticate(stage, inherited, scalar, git_repo=repo)
        scan_sources(stage, deps)
        journal_baseline = CRATE / 'src/context_version_journal/inspection_baseline.rs'
        registration = ('#[path = "context_owner_inspection_model_v1.rs"]\n'
                        'mod owner_inspection_model;\nuse owner_inspection_model::*;\n\n').encode()
        extra = CRATE / 'src/unregistered_inspection_source.rs'
        controls = [
            ('baseline_value', journal_baseline, b'        self.context_generation\n', b'        0\n',
             'independent frozen inspection: context_version_journal'),
            ('baseline_const', journal_baseline, b'const fn baseline_inspection_context_generation_v1',
             b'fn baseline_inspection_context_generation_v1', 'independent frozen inspection: context_version_journal'),
            *[('baseline_projection', CRATE / ('src/' + owner + '/inspection_baseline.rs'), None, b'\n',
               'independent frozen inspection: ' + owner) for owner, _ in OWNERS[1:]],
            *[('adapter_drift', CRATE / 'src' / path, None, b'\n',
               'exact reviewed adapter: ' + str(CRATE / 'src' / path)) for path in ADAPTER_PINS],
            *[('proof_transform', path, None, b'\n', 'exact inherited proof transform: ' + str(path))
              for path in (GUARDS, ACQUIRE, QUERY, DISPOSAL_WITNESSES, RELEASE_WITNESSES)],
            ('inherited_drift', CONSTRUCTOR.BODY, None, b'\n', 'unchanged inherited input: ' + str(CONSTRUCTOR.BODY)),
            ('model_registration', ROOT, registration, b'', 'exact inspection paired root'),
            ('raw_root', RAW, None, b'\n', 'exact inspection actual root'),
            ('macro_roster', BODY, b'macro_rules! owner_inspection_scalar_body {',
             b'macro_rules! unregistered_inspection_body {', 'exact inspection macro roster'),
            ('unregistered_source', extra, None, b'// Unregistered source fixture.\n', 'closed runtime source roster'),
            *[('document_drift', path, None, b'\n', 'exact reviewed document: ' + str(path)) for path in DOCUMENT_PINS],
        ]
        need(len(controls) == 21 + len(DOCUMENT_PINS), 'exact source-auth selftest roster')
        for name, path, before, after, expected_error in controls:
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
                except ValueError as error:
                    need(str(error) == expected_error, 'source-auth rejecting boundary: ' + name + ': ' + str(error))
                    rejected += 1
                else:
                    raise ValueError('accepted source-auth mutation: ' + name)
            finally:
                if original is None:
                    target.unlink()
                else:
                    target.write_bytes(original)
        authenticate(stage, inherited, scalar, git_repo=repo)
    need(rejected == 35 + len(DOCUMENT_PINS), 'exact inspection selftest inventory')
    print(f'PASS: inspection diagnostic, independence, and source-auth checks; {rejected} malformed cases rejected; '
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
    deps = dependencies(repo)
    scalar, legacy, base = (deps[name] for name in ('scalar', 'legacy', 'base'))
    lock = scalar.campaign_lock(output)
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    inherited = inherited_sources(repo, base)
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
    cases = [('positive-before', None), *[(row[0], row) for row in MUTATIONS], ('positive-after', None),
             ('raw-regression', None), ('constructor-regression', None)]
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
    cpu_parser = module(repo, deps['settlement'].CPU_PARSER, 'inspection_cpu_parser').test_results

    def proof_root(name):
        return RAW if name == 'raw-regression' else CONSTRUCTOR.ROOT if name == 'constructor-regression' else ROOT

    def validate(name, row, stdout, stderr):
        if name in ('closure-before', 'closure-after'):
            need(row['command'] == closure_command and row['status'] == 0 and not stderr and stdout ==
                 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'pinned Verus distribution')
        elif name in cpu_commands:
            need(row['command'] == cpu_commands[name] and row['status'] == 0, 'CPU check: ' + name)
            if name == 'test':
                need(cpu_parser(stdout) == CPU_TEST_RESULTS, 'exact CPU test inventory')
            if name == 'recorder-test':
                need(stdout == recorder_transcript() and not stderr, 'recorder regression inventory')
        else:
            root, change = proof_root(name), mutations[name]
            recorded_root = Path(row['command'][-1])
            need(recorded_root.is_absolute() and str(recorded_root).endswith('/' + str(root)), 'resumed proof root')
            stage = Path(str(recorded_root)[:-len(str(root)) - 1])
            need(stage.parent == output and stage.name.startswith('sources-'), 'owned recorded source stage')
            need(row['command'] == command(legacy, verus, stage / root, change), 'exact proof command')
            observed = legacy.normalized(base, stdout, stderr, stage)
            scalar.check_verifier(observed['verus'])
            if change:
                check_negative(scalar, name, row['status'], observed)
            else:
                count = RAW_VERIFIED if name == 'raw-regression' else CONSTRUCTOR_VERIFIED if name == 'constructor-regression' else PAIRED_VERIFIED
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

    for phase in ('before', 'after'):
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
                scan_sources(stage, deps)
                campaign.run(name, command(legacy, verus, stage / proof_root(name), change), proof_timeout(change), env)
                print(name, campaign.proofs[name]['result'], flush=True)
            if stop(name):
                return
        need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']), 'matching whole-root positives')
    for name, cpu_command in cpu:
        campaign.run(name, cpu_command, 600, env)
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
