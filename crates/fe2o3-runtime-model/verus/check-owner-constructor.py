#!/usr/bin/env python3
"""Qualify shared constructors against independent contents and outcome traces.

Developer normal-return contents and observed reservation outcomes only. Native
allocation, physical capacity, partial-object drops and unwind are not proved.
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
ROOT = CRATE / 'verus/context_owner_constructor_paired_v1.rs'
RAW = CRATE / 'verus/context_owner_constructor_execution_v1.rs'
EXECUTION = CRATE / 'verus/context_owner_constructor_execution_bodies_v1.rs'
BODY = CRATE / 'src/context_version_journal/constructor_bodies.rs'
MODEL = CRATE / 'verus/context_owner_constructor_model_v1.rs'
BRIDGE = CRATE / 'verus/context_owner_constructor_correspondence_v1.rs'
WITNESSES = CRATE / 'verus/context_owner_constructor_witnesses_v1.rs'
RELEASE_INVARIANT = CRATE / 'verus/context_read_invariant_v1.rs'
RETIREMENT_WITNESSES = CRATE / 'verus/context_owner_retirement_witnesses_v1.rs'
PREVIOUS = CRATE / 'verus/check-owner-disposal.py'
FROZEN = 'f1022d0d3d22f4773a6b40d8ed1e040051ce512f'
FROZEN_INPUTS = Path('docs/evidence/dev-owner-disposal-2026-09-23/records/source.json')
OWNERS = [('context_version_journal', 'ContextVersionJournalV1', 'journal'),
          ('context_read_leases', 'ContextReadLeasedJournalV1', 'stable'),
          ('context_producer_reads', 'ContextProducerReadJournalV1', 'producer')]
POLICY_NEW = [EXECUTION, MODEL, BRIDGE, WITNESSES]
NEW = [ROOT, RAW, BODY, *POLICY_NEW,
       CRATE / 'src/context_version_journal/construction.rs',
       CRATE / 'src/context_version_journal/tests/construction_probe.rs',
       *[CRATE / ('src/' + owner + '/construction_baseline.rs') for owner, _, _ in OWNERS],
       *[CRATE / ('src/' + owner + '/tests/construction_shared.rs') for owner, _, _ in OWNERS]]
MACROS = ['constructor_' + name + '_body' for name in
          ('entry', 'try', 'reserve', 'vacant', 'free', 'zero', 'journal', 'stable', 'producer')]
PROJECTIONS = [(BODY, 'audited-constructor.rs', 9)]

# Inventories come from terminal whole-root measurements, not scoped sums.
PAIRED_VERIFIED = 1134
RAW_VERIFIED = 370
DISPOSAL_VERIFIED = 1086
CPU_TEST_RESULTS = [(1014, 0, 18, 0, 0), (27, 0, 0, 0, 0)]
ADAPTER_PINS = {
    'context_version_journal.rs': '80b78611302cc547b038b7d43f54e4420c49abbe45d0a1da2d1413d9629d4b68',
    'context_read_leases.rs': '9324e00f83ce6ae9e4c1167e1f89d7d9aa057fd4963d45349ec21c06c470e894',
    'context_producer_reads.rs': 'db9bfd47fd796669e8664e340e7c602190bc8fbf10a94b68a78dfce4df04ed00',
    'context_version_journal/tests.rs': '31b67c3f459cdec86e38c879d0782fea458a4a3279f45b60a420fbad9969d41f',
    'context_read_leases/tests.rs': 'ecf2817dfdb7fbd7a9dae6c6e3d0834b6590b4f165e7f9e0d9f1c66d5930935f',
    'context_producer_reads/tests.rs': '41de121c10fd54b9c3ef830846301e3c1775ab9171f5b35c741bc8f0ffe4776b',
}
NEW_PINS = {
    'context_version_journal/construction.rs': '0a254a81632da68df3a26a4fc21f0533f16f9da6d822ec4c997709e782f863b3',
    'context_version_journal/tests/construction_probe.rs': '1fa2eb87cda7aae376f39aee25ab61ab61dcc1ac65007002e3c0b9397acdc458',
}
DOCUMENT_PINS = {
    Path('docs/runtime-producer-read-reservations-v1.md'): 'dd82024950561b3b3685f602ded204a947a3138b3a614180ad329ec525eed94f',
}
# The release proof is authenticated by an exact frozen-source transform below.
# This pin covers only the reviewed retirement fixture decomposition.
RETIREMENT_WITNESSES_SHA256 = '431bb7487cb7d18bfd6f9cbfb182102e92cd3772662cdce8e0bb63358e978d00'


def mutation(name, macro, function, before, after):
    return (name, BODY, 'macro_rules! constructor_' + macro + '_body {',
            'production', function, before, after, 'execution')


JOURNAL = 'ContextVersionJournalV1::new_with_allocator_v1'
STABLE = 'ContextReadLeasedJournalV1::new_with_allocator_v1'
PRODUCER = 'ContextProducerReadJournalV1::new_with_allocator_v1'
VACANT = 'constructor_vacant_slots_v1'
FREE = 'constructor_free_slots_v1'
ZERO = 'constructor_zero_slots_v1'
SITES = [
    ('journal', JOURNAL, VACANT, '$vacant', '$writers', 0),
    ('journal', JOURNAL, FREE, '$free', '$writers', 1),
    ('journal', JOURNAL, VACANT, '$vacant', '$allocations', 2),
    ('journal', JOURNAL, FREE, '$free', '$allocations', 3),
    ('journal', JOURNAL, VACANT, '$vacant', '$allocations', 4),
    ('journal', JOURNAL, FREE, '$free', '$allocations', 5),
    ('journal', JOURNAL, VACANT, '$vacant', '$allocations', 6),
    ('stable', STABLE, VACANT, '$vacant', '$reads', 7),
    ('stable', STABLE, FREE, '$free', '$reads', 8),
    ('stable', STABLE, ZERO, '$zero', '$allocations', 9),
    ('producer', PRODUCER, VACANT, '$vacant', '$reads', 10),
    ('producer', PRODUCER, FREE, '$free', '$reads', 11),
    ('producer', PRODUCER, ZERO, '$zero', '$allocations', 12),
]
MUTATIONS = []
for owner, function, helper, call, capacity, site in SITES:
    MUTATIONS.extend([
        mutation('failure_site_' + str(site), 'reserve', helper,
                 'if !$allocator.reserve($site, $capacity, &mut $storage) {',
                 'if !$allocator.reserve($site, $capacity, &mut $storage) && $site != ' + str(site) + ' {'),
        mutation('request_site_' + str(site), owner, function,
                 f'{call}({capacity}, {site}, $allocator)', f'{call}({capacity}, 13, $allocator)'),
        mutation('request_capacity_' + str(site), owner, function,
                 f'{call}({capacity}, {site}, $allocator)', f'{call}(0, {site}, $allocator)'),
    ])
MUTATIONS.extend([
    mutation('context_zero', 'journal', JOURNAL, '$context == 0 || $context == u64::MAX', '$context == u64::MAX'),
    mutation('context_max', 'journal', JOURNAL, '$context == 0 || $context == u64::MAX', '$context == 0'),
    mutation('allocation_zero', 'journal', JOURNAL, '$allocations == 0 || ', ''),
    mutation('allocation_max', 'journal', JOURNAL, '$allocations > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1', 'false'),
    mutation('writer_zero', 'journal', JOURNAL, '$writers == 0 || ', ''),
    mutation('writer_max', 'journal', JOURNAL, '$writers > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1', 'false'),
    mutation('reads_zero', 'stable', STABLE, '$reads == 0 || ', ''),
    mutation('reads_max', 'stable', STABLE, '$reads > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1', 'false'),
    mutation('journal_precedence', 'journal', JOURNAL,
             'if $context == 0 || $context == u64::MAX {',
             'if $allocations == 0 { return Err(ContextVersionJournalErrorV1::InvalidCapacity); } '
             'if $context == 0 || $context == u64::MAX {'),
    mutation('reads_precedence', 'stable', STABLE,
             'if $reads == 0 || $reads > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1 {',
             'if $context == 0 { return Err(ContextVersionJournalErrorV1::InvalidContextGeneration); } '
             'if $reads == 0 || $reads > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1 {'),
    mutation('reserve_error', 'reserve', VACANT, 'ContextVersionJournalErrorV1::StorageAllocationFailed',
             'ContextVersionJournalErrorV1::InvalidCapacity'),
    mutation('child_error', 'try', PRODUCER, 'return Err(error);',
             'return Err(ContextVersionJournalErrorV1::InvalidCapacity);'),
    mutation('vacant_initialization', 'vacant', VACANT, 'while $slots.len() < $capacity',
             'while $slots.len() < $capacity && false'),
    mutation('free_order', 'free', FREE, '$free.push($next);', '$free.push(0);'),
    mutation('zero_initialization', 'zero', ZERO, '$counts.push(0);', '$counts.push(1);'),
    mutation('watermark', 'journal', JOURNAL, 'registration_watermark: 0', 'registration_watermark: 1'),
    mutation('reserved_count', 'journal', JOURNAL, 'reserved_count: 0', 'reserved_count: 1'),
    mutation('stable_incarnation', 'stable', STABLE, 'next_incarnation: 1', 'next_incarnation: 0'),
    mutation('producer_incarnation', 'producer', PRODUCER, 'next_incarnation: 1', 'next_incarnation: 0'),
    mutation('reservation_order', 'journal', JOURNAL,
             'let writers = constructor_try_body!($syntax, $vacant($writers, 0, $allocator), [$($failure)*]);\n'
             '            let free = constructor_try_body!($syntax, $free($writers, 1, $allocator), [$($failure)*]);',
             'let free = constructor_try_body!($syntax, $free($writers, 1, $allocator), [$($failure)*]);\n'
             '            let writers = constructor_try_body!($syntax, $vacant($writers, 0, $allocator), [$($failure)*]);'),
])
for owner, ty, prefix in OWNERS:
    MUTATIONS.append(mutation(prefix + '_entry', 'entry', ty + '::new_observed_v1',
        '$construct($($argument,)+ $allocator)', 'Err(ContextVersionJournalErrorV1::InvalidCapacity)'))
    function = 'constructor_' + prefix + '_paired_exec_v1'
    arguments = 'context, allocations, writers' + ('' if prefix == 'journal' else ', reads')
    logical_type = {'journal': 'JournalContentsV1', 'stable': 'ReadContentsV1', 'producer': 'ProducerReadContentsV1'}[prefix]
    MUTATIONS.extend([
        (prefix + '_skip_actual', BRIDGE, 'fn ' + function + '(', 'production', function,
         f'let result = {ty}::new_observed_v1({arguments}, actual);',
         f'let result: Result<{ty}, ContextVersionJournalErrorV1> = Err(ContextVersionJournalErrorV1::InvalidCapacity);', 'execution'),
        (prefix + '_skip_model', BRIDGE, 'fn ' + function + '(', 'production', function,
         f'let model_result = logical::constructor_{prefix}_model_exec_v1(model, {arguments});',
         f'let model_result: Result<logical::{logical_type}, logical::ConstructorErrorV1> = Err(logical::ConstructorErrorV1::InvalidCapacity);', 'execution'),
    ])
for name, before, after in [
    ('reserve_outcome', 'let result = if index < self.outcomes.len() { self.outcomes[index] } else { false };', 'let result = true;'),
    ('reserve_index', 'let index = self.attempts.len();', 'let index = 0;'),
    ('exhausted_outcome', 'else { false };', 'else { true };'),
    ('reserve_attempt', 'self.attempts.push((site, capacity));', 'self.attempts.push((site, 0));'),
    ('reserve_prefix', 'self.attempts.push((site, capacity));', 'self.attempts = Vec::new(); self.attempts.push((site, capacity));'),
]:
    MUTATIONS.append((name, EXECUTION, 'impl ConstructorObservationsV1 {', 'production',
                      'ConstructorObservationsV1::reserve', before, after, 'execution'))
MUTATIONS.append(('independent_attempt', MODEL, 'pub fn constructor_reserve_model_exec_v1(',
                  'owner_constructor_model', 'constructor_reserve_model_exec_v1',
                  'observations.attempts.push((site, capacity));', '', 'execution'))


def need(condition, message):
    if not condition:
        raise ValueError(message)


def module(repo, path, name):
    spec = importlib.util.spec_from_file_location(name, repo / path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def dependencies(repo):
    result, path = {}, PREVIOUS
    for name in ('disposal', 'retirement', 'scalar', 'history', 'settlement', 'writer', 'enrollment', 'unknown'):
        result[name] = module(repo, path, 'constructor_' + name)
        if name != 'unknown':
            path = result[name].PREVIOUS
    result['legacy'] = module(repo, result['settlement'].LEGACY, 'constructor_legacy')
    result['base'] = module(repo, result['legacy'].BASE, 'constructor_recorder')
    result['policy'] = module(repo, result['legacy'].POLICY, 'constructor_policy')
    return result


def configuration():
    for name, value in [('PAIRED_VERIFIED', PAIRED_VERIFIED), ('RAW_VERIFIED', RAW_VERIFIED),
                        ('DISPOSAL_VERIFIED', DISPOSAL_VERIFIED)]:
        need(type(value) is int and value > 0, 'final measured count required: ' + name)
    need(type(CPU_TEST_RESULTS) is list and len(CPU_TEST_RESULTS) == 2
         and all(type(row) is tuple and len(row) == 5 and all(type(n) is int and n >= 0 for n in row)
                 for row in CPU_TEST_RESULTS), 'final unit/doctest inventory required')
    for pins in (ADAPTER_PINS, NEW_PINS, DOCUMENT_PINS):
        need(all(type(pin) is str and re.fullmatch(r'[0-9a-f]{64}', pin) for pin in pins.values()),
             'final reviewed source pins required')
    need(type(RETIREMENT_WITNESSES_SHA256) is str
         and re.fullmatch(r'[0-9a-f]{64}', RETIREMENT_WITNESSES_SHA256), 'reviewed retirement proof pin required')


def replace_once(source, before, after):
    need(source.count(before) == 1, 'unique frozen constructor redirect: ' + before)
    return source.replace(before, after, 1)


def function(source, name, indent=''):
    anchors = list(re.finditer(r'^' + indent + r'(?:pub )?fn ' + re.escape(name) + r'(?:<[^\n]+>)?\(', source, re.M))
    need(len(anchors) == 1, 'unique frozen function: ' + name)
    start = anchors[0].start()
    end = source.index('\n' + indent + '}\n', start) + len('\n' + indent + '}')
    return source[start:end]


def frozen_helper(source, name, variable):
    body = function(source, name)
    generic = '' if name == 'free_slots' else '<T>'
    before = f'fn {name}{generic}(capacity: usize)'
    after = (f'fn baseline_{name}_v1{generic}(\n    capacity: usize,\n    site: u8,\n'
             '    allocator: &mut impl ConstructorAllocatorV1,\n)')
    body = replace_once(body, before, after)
    pattern = (r'    ' + variable + r'\s*\.try_reserve_exact\(capacity\)\s*'
               r'\.map_err\(\|_\| ContextVersionJournalErrorV1::StorageAllocationFailed\)\?;')
    body, count = re.subn(pattern, '    allocator\n        .reserve(site, capacity, &mut ' + variable + ')\n'
                         '        .then_some(())\n        .ok_or(ContextVersionJournalErrorV1::StorageAllocationFailed)?;', body)
    need(count == 1, 'one frozen allocation hook: ' + name)
    return body


def frozen_baseline(source, owner, ty, prefix):
    body = function(source, 'new', '    ')
    body = replace_once(body, '    pub fn new(', '    pub(crate) fn baseline_new_with_allocator_v1(')
    last = 'writer_capacity' if prefix == 'journal' else 'reads'
    body = replace_once(body, f'        {last}: usize,\n',
                        f'        {last}: usize,\n        allocator: &mut impl ConstructorAllocatorV1,\n')
    if prefix == 'journal':
        ident = function(source, 'issuable_context_id').replace('fn issuable_context_id(', 'fn baseline_issuable_context_id_v1(', 1)
        helpers = [ident, frozen_helper(source, 'vacant_slots', 'slots'), frozen_helper(source, 'free_slots', 'free')]
        body = replace_once(body, 'issuable_context_id(context_generation)', 'baseline_issuable_context_id_v1(context_generation)')
        fields = [('writers', 'vacant_slots', 'writer_capacity'), ('free', 'free_slots', 'writer_capacity'),
                  ('allocations', 'vacant_slots', 'allocation_capacity'), ('allocation_free', 'free_slots', 'allocation_capacity'),
                  ('members', 'vacant_slots', 'allocation_capacity'), ('member_free', 'free_slots', 'allocation_capacity'),
                  ('scratch', 'vacant_slots', 'allocation_capacity')]
        for site, (field, helper, capacity) in enumerate(fields):
            body = replace_once(body, f'{field}: {helper}({capacity})?',
                                f'{field}: baseline_{helper}_v1({capacity}, {site}, allocator)?')
        imported = 'construction::ConstructorAllocatorV1'
    else:
        helpers = [frozen_helper(source, 'storage', 'result')]
        child, field = ('ContextVersionJournalV1', 'journal') if prefix == 'stable' else ('ContextReadLeasedJournalV1', 'stable')
        args = ['generation', 'allocations', 'writers'] + ([] if prefix == 'stable' else ['reads'])
        body = replace_once(body, f'let {field} = {child}::new(' + ', '.join(args) + ')?;',
                            f'let {field} = {child}::baseline_new_with_allocator_v1(\n'
                            + ''.join('            ' + argument + ',\n' for argument in [*args, 'allocator']) + '        )?;')
        names = ['leases', 'free_reads', 'readers'] if prefix == 'stable' else ['reservations', 'free', 'counts']
        for site, (name, capacity) in enumerate(zip(names, ['reads', 'reads', 'allocations']), 7 if prefix == 'stable' else 10):
            body = replace_once(body, f'let mut {name} = storage({capacity})?;',
                                f'let mut {name} = baseline_storage_v1({capacity}, {site}, allocator)?;')
        imported = 'crate::context_version_journal::construction::ConstructorAllocatorV1'
    return ('// Frozen at ' + FROZEN + '; reserve hooks and names only.\nuse super::*;\nuse ' + imported + ';\n\n'
            + '\n\n'.join(helpers) + '\n\nimpl ' + ty + ' {\n' + body + '\n}\n')


def independent_model(source, policy):
    code = policy.code_only(source)
    need(re.fullmatch(r'\s*use\s+super\s*::\s*\*\s*;\s*verus\s*!\s*\{.*\}\s*', code, re.S) is not None,
         'exact independent model import/envelope')
    need(len(re.findall(r'\buse\b', code)) == 1, 'no independent model import aliases')
    need(re.findall(r'\b([A-Za-z_][A-Za-z0-9_]*)\s*!\s*[({\[]', code) == ['verus'],
         'no shared macro invocation in independent model')
    need(re.search(r'\bproduction\b|\bshared_[A-Za-z0-9_]+\b|\b[A-Za-z_][A-Za-z0-9_]*_body\b'
                   r'|\bmacro_export\b|\bmacro_use\b', code) is None,
         'independent model cannot reference production or shared bodies')


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
    added = {CRATE / 'src' / path for path in NEW_PINS}
    need(changed <= inherited and added <= set(NEW) and not added.intersection(inherited), 'reviewed runtime path classes')
    need(set(DOCUMENT_PINS) <= inherited and not changed.intersection(DOCUMENT_PINS), 'reviewed document path classes')
    proofs = {RELEASE_INVARIANT, RETIREMENT_WITNESSES}
    need(len(proofs) == 2 and proofs <= inherited
         and not proofs.intersection(changed | added | set(DOCUMENT_PINS)), 'closed reviewed inherited proof paths')
    for path in inherited:
        current = (repo / path).read_bytes()
        if path in changed:
            need(scalar.digest(current) == ADAPTER_PINS[str(path.relative_to(CRATE / 'src'))], 'exact reviewed adapter: ' + str(path))
        elif path in DOCUMENT_PINS:
            need(scalar.digest(current) == DOCUMENT_PINS[path], 'exact reviewed document: ' + str(path))
        elif path == RELEASE_INVARIANT:
            expected = replace_once(old(path).decode(), '\npub proof fn release_arena_prefix_v1(',
                                    '\n#[verifier::spinoff_prover]\npub proof fn release_arena_prefix_v1(')
            need(current == expected.encode(), 'exact isolated release proof: ' + str(path))
        elif path == RETIREMENT_WITNESSES:
            need(scalar.digest(current) == RETIREMENT_WITNESSES_SHA256,
                 'exact reviewed retirement proof: ' + str(path))
        else:
            need(current == old(path), 'unchanged inherited input: ' + str(path))
    for path in added:
        need(scalar.digest((repo / path).read_bytes()) == NEW_PINS[str(path.relative_to(CRATE / 'src'))],
             'exact reviewed constructor helper: ' + str(path))
    for owner, ty, prefix in OWNERS:
        expected = frozen_baseline(old(CRATE / ('src/' + owner + '.rs')).decode(), owner, ty, prefix)
        need((repo / CRATE / ('src/' + owner + '/construction_baseline.rs')).read_text() == expected,
             'independent frozen constructor: ' + owner)
    expected = old(CRATE / 'verus/context_owner_disposal_paired_v1.rs').decode()
    expected = replace_once(expected, 'mod production {',
        '#[path = "context_owner_constructor_model_v1.rs"]\nmod owner_constructor_model;\nuse owner_constructor_model::*;\n\nmod production {')
    expected = replace_once(expected, 'include!("context_owner_disposal_execution_v1.rs");',
                            'include!("context_owner_constructor_execution_v1.rs");')
    expected = replace_once(expected, '    include!("context_owner_disposal_witnesses_v1.rs");',
        '    include!("context_owner_disposal_witnesses_v1.rs");\n'
        '    include!("context_owner_constructor_correspondence_v1.rs");\n'
        '    include!("context_owner_constructor_witnesses_v1.rs");')
    need((repo / ROOT).read_text() == expected, 'exact constructor paired root')
    expected = ('// Actual declarations and shared construction; physical allocation remains separate.\n'
                'include!("context_owner_disposal_execution_v1.rs");\n'
                'include!("../src/context_version_journal/constructor_bodies.rs");\n'
                'include!("context_owner_constructor_execution_bodies_v1.rs");\n')
    need((repo / RAW).read_text() == expected, 'exact constructor actual root')
    need(re.findall(r'^macro_rules! (\w+) \{', (repo / BODY).read_text(), re.M) == MACROS, 'exact constructor macro roster')
    policy = module(repo, Path('examples/wave64_collectives_v1/check-proof-source.py'), 'constructor_independence_policy')
    independent_model((repo / MODEL).read_text(), policy)
    source_roster(repo, inherited)


def check_negative(scalar, name, status, observed):
    need(type(status) is int and status == 1, 'normal negative exit')
    scalar.negative_report(observed['result'])
    errors = [row for row in observed['diagnostics'] if row['level'] == 'error']
    allowed = {'assertion failed', 'postcondition not satisfied', 'precondition not satisfied',
               'invariant not satisfied before loop', 'invariant not satisfied at end of loop body'}
    need(any(row['message'] in allowed for row in errors)
         and all(row['message'] in allowed or re.fullmatch(r'aborting due to \d+ previous errors?', row['message'])
                 for row in errors), 'logical failure, not arithmetic, syntax, timeout, or solver resource exhaustion')


def inherited_sources(repo, base):
    data = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(FROZEN_INPUTS)], text=True)
    return {Path(p) for p in base.unique_json(data)['inputs']} | {FROZEN_INPUTS}


def scan_sources(stage, deps):
    legacy, disposal = deps['legacy'], deps['disposal']
    policy, ancestor = deps['policy'], deps['unknown']
    leaves = set(legacy.POLICY_SOURCES) | (set(ancestor.NEW) - {ancestor.ROOT, ancestor.RAW, ancestor.BODY})
    for name in ('enrollment', 'writer', 'settlement', 'history', 'scalar', 'retirement', 'disposal'):
        leaves.update(deps[name].POLICY_NEW)
    for path in leaves | set(POLICY_NEW):
        disposal.scan_leaf(stage, path, policy)
    independent_model((stage / MODEL).read_text(), policy)
    projections = [*legacy.PROJECTIONS, (ancestor.BODY, 'audited-unknown-wrappers.rs', 3)]
    for name in ('enrollment', 'writer', 'settlement', 'scalar', 'retirement', 'disposal'):
        projections.extend(deps[name].PROJECTIONS)
    for path, projection, count in [*projections, *PROJECTIONS]:
        data = (stage / path).read_text()
        need(data.count('macro_rules!') == count, 'macro roster')
        target = stage / projection
        target.write_text(data.replace('macro_rules!', ''))
        policy.scan(target)


def recorder_transcript():
    return ('PASS: receipt and checkpoint replay checks; 29 malformed or conflicting operations rejected\n'
            'PASS: retirement negative diagnostic policy; 8 malformed cases rejected\n'
            f'PASS: constructor diagnostic, independence, and source-auth checks; {23 + len(DOCUMENT_PINS)} malformed cases rejected; '
            f'{len(MUTATIONS)} scoped mutation anchors authenticated\n')


def recorder_selftest():
    repo = Path(__file__).resolve().parents[3]
    deps = dependencies(repo)
    deps['retirement'].recorder_selftest()
    scalar, policy, legacy, base = (deps[name] for name in ('scalar', 'policy', 'legacy', 'base'))
    valid = {'result': {'encountered-error': True, 'encountered-vir-error': False, 'verified': 0,
                        'errors': 1, 'is-verifying-entire-crate': False},
             'diagnostics': [{'level': 'error', 'message': 'postcondition not satisfied'}]}
    for message in ('assertion failed', 'postcondition not satisfied', 'precondition not satisfied',
                    'invariant not satisfied before loop', 'invariant not satisfied at end of loop body'):
        row = copy.deepcopy(valid)
        row['diagnostics'][0]['message'] = message
        check_negative(scalar, 'failure_site_0', 1, row)
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
            check_negative(scalar, 'failure_site_0', status, row)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted invalid constructor negative: ' + fault)
    prefix, suffix = 'use super::*;\nverus! {\n', '\n}\n'
    independent_model(prefix + 'pub fn sample() { if !false { assert(true); } }' + suffix, policy)
    independent_model('// production::constructor_entry_body!()\n' + prefix + 'pub fn sample() {}' + suffix, policy)
    for body in ('constructor_entry_body!();', 'production::run();', 'shared_constructor_vacant_v1();',
                 'use super::production as other;', 'alias!{}', 'constructor_entry_body ();'):
        try:
            independent_model(prefix + 'pub fn sample() { ' + body + ' }' + suffix, policy)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted coupled independent model: ' + body)
    need(len({row[0] for row in MUTATIONS}) == len(MUTATIONS), 'unique mutation names')
    for row in MUTATIONS:
        legacy.mutated((repo / row[1]).read_bytes(), row)
    inherited = inherited_sources(repo, base)
    pin_sets = [(ADAPTER_PINS, ADAPTER_PINS.copy()), (NEW_PINS, NEW_PINS.copy()),
                (DOCUMENT_PINS, DOCUMENT_PINS.copy())]
    try:
        # Selftest fixture only. Qualification configuration never fills pins.
        for pins, saved in pin_sets:
            for path, pin in saved.items():
                if pin is None:
                    target = repo / path if pins is DOCUMENT_PINS else repo / CRATE / 'src' / path
                    pins[path] = scalar.digest(target.read_bytes())
        with tempfile.TemporaryDirectory(prefix='constructor-source-selftest-') as temporary:
            stage = Path(temporary)
            for path in sorted(inherited | set(NEW)):
                target = stage / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((repo / path).read_bytes())
            authenticate(stage, inherited, scalar, git_repo=repo)
            scan_sources(stage, deps)
            baseline = CRATE / 'src/context_version_journal/construction_baseline.rs'
            adapter = CRATE / 'src/context_version_journal.rs'
            helper = CRATE / 'src/context_version_journal/construction.rs'
            unchanged = deps['disposal'].BODY
            extra = CRATE / 'src/unregistered_constructor_source.rs'
            registration = ('#[path = "context_owner_constructor_model_v1.rs"]\n'
                            'mod owner_constructor_model;\nuse owner_constructor_model::*;\n\n').encode()
            controls = [
                ('baseline_drift', baseline, b'if !baseline_issuable_context_id_v1(context_generation) {', b'if false {',
                 'independent frozen constructor: context_version_journal'),
                ('baseline_helper_drift', baseline, b'value != 0 && value != u64::MAX', b'value != 0',
                 'independent frozen constructor: context_version_journal'),
                ('adapter_drift', adapter, None, b'\n', 'exact reviewed adapter: ' + str(adapter)),
                ('native_helper_drift', helper, None, b'\n', 'exact reviewed constructor helper: ' + str(helper)),
                ('inherited_drift', unchanged, None, b'\n', 'unchanged inherited input: ' + str(unchanged)),
                ('release_proof_drift', RELEASE_INVARIANT,
                 b'#[verifier::spinoff_prover]\npub proof fn release_arena_prefix_v1(',
                 b'pub proof fn release_arena_prefix_v1(', 'exact isolated release proof: ' + str(RELEASE_INVARIANT)),
                ('retirement_proof_drift', RETIREMENT_WITNESSES, None, b'\n',
                 'exact reviewed retirement proof: ' + str(RETIREMENT_WITNESSES)),
                ('model_registration', ROOT, registration, b'', 'exact constructor paired root'),
                ('unregistered_source', extra, None, b'// Unregistered source fixture.\n', 'closed runtime source roster'),
                *[('document_drift', path, None, b'\n', 'exact reviewed document: ' + str(path)) for path in DOCUMENT_PINS],
            ]
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
    finally:
        for pins, saved in pin_sets:
            pins.clear()
            pins.update(saved)
    need(rejected == 23 + len(DOCUMENT_PINS), 'exact constructor selftest inventory')
    print(f'PASS: constructor diagnostic, independence, and source-auth checks; {rejected} malformed cases rejected; '
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
    scalar, legacy, base, disposal = (deps[name] for name in ('scalar', 'legacy', 'base', 'disposal'))
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
             ('raw-regression', None), ('disposal-regression', None)]
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
    cpu_parser = module(repo, deps['settlement'].CPU_PARSER, 'constructor_cpu_parser').test_results

    def proof_root(name):
        return RAW if name == 'raw-regression' else disposal.ROOT if name == 'disposal-regression' else ROOT

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
            need(row['command'] == legacy.command(verus, stage / root, change), 'exact proof command')
            observed = legacy.normalized(base, stdout, stderr, stage)
            scalar.check_verifier(observed['verus'])
            if change:
                check_negative(scalar, name, row['status'], observed)
            else:
                count = RAW_VERIFIED if name == 'raw-regression' else DISPOSAL_VERIFIED if name == 'disposal-regression' else PAIRED_VERIFIED
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
