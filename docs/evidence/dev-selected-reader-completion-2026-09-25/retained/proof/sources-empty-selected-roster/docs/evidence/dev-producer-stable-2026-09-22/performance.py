#!/usr/bin/env python3
"""Authenticate frozen stable wrappers and replay instrumented CPU measurements."""
from pathlib import Path
import hashlib
import re
import statistics
import subprocess

BASELINE = '30aac7720c4a2eef34da3e3682ec5b74784ed61d'
SRC = Path('crates/fe2o3-runtime-model/src')
TEST = 'context_producer_reads::tests::stable_wrappers_shared::performance::stable_wrapper_performance'


def need(value, message):
    if not value:
        raise ValueError(message)


def method(source, name):
    anchors = [prefix + name + '(' for prefix in ('    pub fn ', '    pub const fn ', '    pub(crate) fn ', '    fn ')]
    anchors = [anchor for anchor in anchors if anchor in source]
    need(len(anchors) == 1 and source.count(anchors[0]) == 1, 'unique source method: ' + name)
    start = source.index(anchors[0])
    return source[start:source.index('\n    }\n', start) + len('\n    }\n')]


def historical_projection(source):
    declarations = list(re.finditer(r'^pub (?:(?:open spec|proof) )?fn ([A-Za-z0-9_]+)', source, re.M))
    names = [match[1] for match in declarations]
    selected = [
        'producer_chain_frame_v1', 'producer_custody_frame_v1', 'producer_arena_journal_frame_v1',
        'stable_acquire_preserves_producer_invariant_v1', 'stable_release_preserves_producer_invariant_v1',
        'stable_wrapper_acquire_header_v1', 'stable_wrapper_acquire_header_exec_v1',
        'stable_wrapper_acquire_decision_v1', 'stable_wrapper_acquire_relation_v1',
        'stable_wrapper_acquire_exec_v1', 'stable_wrapper_release_exec_v1',
    ]
    pieces = []
    for name in selected:
        index = names.index(name)
        piece = source[declarations[index].start():declarations[index + 1].start()]
        envelope = '\n}\n\nverus! {\n\n'
        if piece.endswith(envelope):
            piece = piece[:-len(envelope)] + '\n'
        pieces.append(piece.rstrip() + '\n')
    return ('// Exact stable-wrapper declarations from the historical lifecycle, in the importing type universe.\n'
            'use super::*;\n\nverus! {\n\n' + '\n'.join(pieces) + '\n}\n')


def authenticate(repo, tree):
    def original(path):
        return subprocess.check_output(['git', '-C', str(repo), 'show', BASELINE + ':' + str(path)]).decode('ascii')

    path = SRC / 'context_producer_reads.rs'
    old = original(path)
    frozen = [method(old, name).replace('pub fn ' + name + '(', 'pub(crate) fn baseline_' + name + '_v1(').rstrip()
              for name in ('acquire_reads', 'release_reads')]
    expected = ('// Frozen from ' + BASELINE + '; only method names/visibility change.\n'
                'use super::*;\n\nimpl ContextProducerReadJournalV1 {\n' + '\n\n'.join(frozen) + '\n}\n')
    need((tree / SRC / 'context_producer_reads/stable_wrapper_baseline.rs').read_text() == expected, 'exact frozen outer methods')
    current = (tree / path).read_text()
    for name in ('validate_read_capacity', 'remaining_read_slots', 'retained_read_count', 'acquire_producer_reads', 'release_producer_reads'):
        need(method(current, name) == method(old, name), 'unchanged producer dependency: ' + name)
    stable = (tree / SRC / 'context_read_leases.rs').read_text()
    addition = '#[cfg(test)]\npub(crate) use guard_test_support::{StableReadFaultV1, StableReadResetV1};\n\n'
    need(stable.count(addition) == 1 and stable.replace(addition, '') == original(SRC / 'context_read_leases.rs'),
         'stable owner unchanged except test-only opaque support exports')
    for relative in [
        'context_read_leases/declarations.rs', 'context_read_leases/acquire.rs',
        'context_read_leases/acquire_bodies.rs', 'context_read_leases/acquire_declarations.rs',
        'context_read_leases/release.rs', 'context_read_leases/release_bodies.rs',
        'context_read_leases/release_declarations.rs', 'context_read_leases/count_bodies.rs',
        'context_producer_reads/declarations.rs', 'context_producer_reads/query.rs',
        'context_producer_reads/query_bodies.rs', 'context_producer_reads/acquire.rs',
        'context_producer_reads/acquire_bodies.rs', 'context_producer_reads/acquire_declarations.rs',
        'context_producer_reads/release.rs', 'context_producer_reads/release_bodies.rs',
        'context_version_journal.rs', 'context_version_journal/declarations.rs',
        'context_version_journal/lookup_bodies.rs', 'context_version_journal/retained.rs',
        'context_version_journal/retained_bodies.rs', 'context_version_journal/writer_lookup_bodies.rs',
    ]:
        need((tree / SRC / relative).read_text() == original(SRC / relative), 'unchanged frozen dependency: ' + relative)
    need('#[cfg(test)]\nmod stable_wrapper_baseline;' in current, 'test-only baseline')
    for relative, pin in {
        'src/context_producer_reads.rs': 'b9159205ab506eed2f6243044293af28a15d6f8df480b132a1563e82447e1423',
        'src/context_producer_reads/stable_wrappers.rs': '0803d0e2a1b89d9f7dc7c756cd7b2e73e5e8c74a51dfaf53354936ea421ce0e8',
        'verus/context_producer_stable_bodies_v1.rs': '709ba5dd8c7853375a1e4eba249119b0b7f4d9cd5b70139fb02397b946cb72a0',
    }.items():
        need(hashlib.sha256((tree / SRC.parent / relative).read_bytes()).hexdigest() == pin,
             'reviewed public/private wrapper and observation adapter: ' + relative)
    path = SRC.parent / 'verus/context_producer_read_lifecycle_v1.rs'
    historical = (tree / path).read_text()
    need(historical == original(path), 'unchanged historical provenance')
    need((tree / SRC.parent / 'verus/context_producer_stable_historical_bodies_v1.rs').read_text()
         == historical_projection(historical), 'exact eleven-declaration historical projection')


def cases():
    result = []
    for operation in ('acquire', 'release'):
        for shape in ('grouped', 'distinct'):
            for kind in ('submission', 'synchronous'):
                result.extend((operation, k, max(k + 2, 1024), shape, kind, 'none') for k in (1, 8, 64, 512, 4096))
                result.append((operation, 8, 65536, shape, kind, 'none'))
            faults = ('reference', 'count', 'evidence', 'capacity') if operation == 'release' else ('budget', 'output', 'extent', 'slot')
            result.extend((operation, 8, 1024, shape, 'submission', fault) for fault in faults)
    return result


def accesses(k, fault):
    return 0 if fault in ('budget', 'output', 'evidence', 'capacity') else k - (fault == 'reference')


def parse(text):
    rows = []
    prefix = 'test ' + TEST + ' ... '
    for line in text.splitlines():
        if line.startswith(prefix):
            line = line[len(prefix):]
        if 'producer_stable_' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 11 and parts[0] in ('producer_stable_acquire', 'producer_stable_release'), 'benchmark row shape')
        for index in (1, 2, 6, 8, 9, 10):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((parts[0].removeprefix('producer_stable_'), int(parts[1]), int(parts[2]), parts[3], parts[4], parts[5],
                     int(parts[6]), parts[7], int(parts[8]), int(parts[9]), int(parts[10])))
    expected = [(*case, round_, variant, min(8192, max(64, 131072 // case[1])), accesses(case[1], case[5]))
                for case in cases() for round_ in range(7) for turn in range(2)
                for variant in ['shared' if (round_ + turn) % 2 == 0 else 'baseline']]
    need(len(rows) == len(expected) == 896, 'complete benchmark row roster')
    grouped = {}
    for row, wanted in zip(rows, expected):
        need((*row[:9], row[10]) == wanted, 'exact ordered benchmark fields')
        need(row[9] > 0, 'positive elapsed time')
        grouped.setdefault(row[:6], {}).setdefault(row[7], []).append(row[9] / row[8])
    return grouped


def table(text):
    lines = ['| Operation | Roster | Capacity | Shape | Consumer | Fault | Shared ns | Frozen ns | Shared/frozen |',
             '| --- | ---: | ---: | --- | --- | --- | ---: | ---: | ---: |']
    for key, rows in parse(text).items():
        shared, baseline = (statistics.median(rows[name]) for name in ('shared', 'baseline'))
        lines.append('| %s | %d | %d | %s | %s | %s | %.2f | %.2f | %.3f |' % (*key, shared, baseline, shared / baseline))
    return '\n'.join(lines)


def selftest(text):
    lines = text.splitlines()
    indices = [i for i, line in enumerate(lines) if 'producer_stable_' in line]
    first = indices[0]
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, '01'), (2, '1'), (3, 'wrong'), (4, 'wrong'),
                         (5, 'wrong'), (6, '1'), (7, 'baseline'), (8, '1'), (9, '0'), (10, '0')):
        changed = list(lines)
        prefix, row = changed[first].split('producer_stable_', 1)
        parts = ('producer_stable_' + row).split(',')
        parts[field] = value
        changed[first] = prefix + ','.join(parts)
        variants.append(changed)
    changed = list(lines)
    changed[first], changed[indices[1]] = changed[indices[1]], changed[first]
    variants.append(changed)
    for operation, fault, variant in [('acquire', 'budget', 'baseline'), ('acquire', 'extent', 'shared'),
                                      ('release', 'reference', 'baseline'), ('release', 'capacity', 'shared'),
                                      ('release', 'none', 'baseline')]:
        changed = list(lines)
        index = next(i for i in indices if 'producer_stable_' + operation + ',' in lines[i]
                     and ',grouped,submission,' + fault + ',' in lines[i] and ',' + variant + ',' in lines[i])
        prefix, row = changed[index].split('producer_stable_', 1)
        parts = ('producer_stable_' + row).split(',')
        parts[10] = '99'
        changed[index] = prefix + ','.join(parts)
        variants.append(changed)
    for changed in variants:
        try:
            parse('\n'.join(changed))
        except ValueError:
            pass
        else:
            raise ValueError('accepted malformed benchmark rows')
    print('PASS: rejected', len(variants), 'altered benchmark row sets')
