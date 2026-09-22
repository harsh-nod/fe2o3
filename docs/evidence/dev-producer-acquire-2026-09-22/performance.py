#!/usr/bin/env python3
"""Authenticate frozen producer admission and replay instrumented CPU measurements."""
import hashlib
from pathlib import Path
import re
import statistics
import subprocess

BASELINE = '4c237366268542e1a19542b25c00f7b58d9f24a8'
SRC = Path('crates/fe2o3-runtime-model/src')
TEST = 'context_producer_reads::tests::acquire_shared::performance::shared_producer_acquire_performance'


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
    starts = []
    for match in declarations:
        start = match.start()
        attribute = '#[verifier::spinoff_prover]\n'
        if source[:start].endswith(attribute):
            start -= len(attribute)
        starts.append(start)
    blocks = [
        ('producer_live_reads_v1', 'producer_live_reads_v1'),
        ('producer_capacity_arithmetic_v1', 'producer_arena_insert_v1'),
        ('producer_outer_frame_v1', 'producer_outer_frame_v1'),
        ('producer_capacity_decision_v1', 'producer_acquire_preflight_exec_v1'),
        ('producer_request_count_v1', 'producer_acquire_commit_exec_v1'),
        ('producer_validated_same_slot_local_v1', 'producer_acquire_preflight_ready_v1'),
        ('producer_fresh_entry_valid_v1', 'producer_fresh_entry_valid_v1'),
        ('producer_acquired_counts_v1', 'producer_acquire_arena_prefix_v1'),
        ('producer_contents_frame_v1', 'producer_acquire_execution_relation_v1'),
        ('producer_acquire_preserves_v1', 'producer_acquire_preserves_v1'),
        ('producer_acquire_contents_exec_v1', 'producer_acquire_contents_exec_v1'),
        ('producer_acquire_exec_v1', 'producer_acquire_exec_v1'),
    ]
    pieces, count = [], 0
    for first, last in blocks:
        begin, end = names.index(first), names.index(last)
        count += end - begin + 1
        piece = source[starts[begin]:starts[end + 1]]
        envelope = '\n}\n\nverus! {\n\n'
        if piece.endswith(envelope):
            piece = piece[:-len(envelope)] + '\n'
        pieces.append(piece)
    need(count == 43, 'exact historical declaration roster')
    return ('// Exact declaration blocks from the historical lifecycle, sharing the importing type universe.\n'
            'use super::*;\n\nverus! {\n\n' + ''.join(pieces).rstrip() + '\n\n}\n')


def authenticate(repo, tree):
    def original(path, commit=BASELINE):
        return subprocess.check_output(['git', '-C', str(repo), 'show', commit + ':' + str(path)]).decode('ascii')

    for owner, typename, names, baseline in [
        ('context_producer_reads', 'ContextProducerReadJournalV1',
         ['retained_producer_read_count', 'retained_read_count', 'remaining_read_slots',
          'validate_read_capacity', 'validate_producer_read_capacity', 'acquire_producer_reads'], 'acquire_baseline.rs'),
        ('context_read_leases', 'ContextReadLeasedJournalV1', ['retained_read_count'], 'acquire_count_baseline.rs'),
    ]:
        old = original(SRC / (owner + '.rs'))
        methods = '\n'.join(method(old, name) for name in names)
        for name in names:
            methods = methods.replace('    pub fn ' + name + '(', '    pub(crate) fn baseline_' + name + '_v1(')
            methods = methods.replace('.' + name + '(', '.baseline_' + name + '_v1(')
        methods = methods.replace('self.stable.baseline_retained_read_count_v1() + self.baseline_retained_producer_read_count_v1()',
                                  'self.stable.baseline_retained_read_count_v1()\n            + self.baseline_retained_producer_read_count_v1()')
        methods = methods.replace('fn baseline_validate_read_capacity_v1(&self, count: usize) ->',
                                  'fn baseline_validate_read_capacity_v1(\n        &self,\n        count: usize,\n    ) ->')
        expected = ('// Frozen from ' + BASELINE + '; only method names/visibility change.\n'
                    'use super::*;\n\nimpl ' + typename + ' {\n' + methods.rstrip() + '\n}\n')
        need((tree / SRC / owner / baseline).read_text() == expected, 'exact frozen admission: ' + owner)
    stable = SRC / 'context_read_leases/acquire_baseline.rs'
    need((tree / stable).read_text() == original(stable), 'unchanged independently frozen stable capacity leaf')
    old_capacity = method(original(SRC / 'context_read_leases.rs', '72bcb1da989ea79c2c48085f5a6c9ba3ce1633f1'),
                          'validate_read_capacity')
    expected_capacity = old_capacity.replace('pub fn validate_read_capacity(', 'pub(crate) fn baseline_validate_read_capacity_v1(')
    expected_capacity = expected_capacity.replace('fn baseline_validate_read_capacity_v1(&self, count: usize) ->',
                                                  'fn baseline_validate_read_capacity_v1(\n        &self,\n        count: usize,\n    ) ->')
    need(method((tree / stable).read_text(), 'baseline_validate_read_capacity_v1') == expected_capacity,
         'exact older stable capacity provenance')
    for relative, anchor in [
        ('context_producer_reads.rs', 'impl Deref for ContextProducerReadJournalV1 {'),
        ('context_read_leases.rs', 'impl Deref for ContextReadLeasedJournalV1 {'),
    ]:
        def block(text):
            start = text.index(anchor)
            return text[start:text.index('\n}\n', start) + 3]
        need(block((tree / SRC / relative).read_text()) == block(original(SRC / relative)), 'unchanged receiver projection')
    for relative in [
        'context_read_leases/declarations.rs', 'context_read_leases/acquire_bodies.rs',
        'context_producer_reads/declarations.rs', 'context_producer_reads/query.rs',
        'context_producer_reads/query_bodies.rs', 'context_version_journal.rs',
        'context_version_journal/declarations.rs', 'context_version_journal/lookup_bodies.rs',
        'context_version_journal/retained.rs', 'context_version_journal/retained_bodies.rs',
        'context_version_journal/writer_lookup_bodies.rs',
    ]:
        need((tree / SRC / relative).read_text() == original(SRC / relative), 'unchanged baseline dependency: ' + relative)
    for name in ['validate_producer_read', 'status']:
        path = SRC / 'context_producer_reads.rs'
        need(method((tree / path).read_text(), name) == method(original(path), name), 'unchanged query adapter')
    path = SRC / 'context_producer_reads.rs'
    def read_key(source):
        start = source.index('fn read_key(')
        return source[start:source.index('\n}\n', start) + 3]
    need(read_key((tree / path).read_text()) == read_key(original(path)), 'unchanged frozen ordering helper')
    for relative, pin in {
        'src/context_producer_reads.rs': 'e7ea57bc60b1be9ca5c0d5cfb9434d9b59acfefef4e32141dceb218efd3db0e7',
        'src/context_read_leases.rs': '491fe54ca6c8adef40cfe1d233a637a2575e3d0bba9746a4f9ae282bc2587288',
        'src/context_producer_reads/acquire.rs': 'a17ea77af264d6c4d25b7af87e97db219edf257d007e0b1a31d2ef137794af94',
        'src/context_producer_reads/acquire_declarations.rs': '1417216d76689f1026696bda590baa7eb59acbd1d36eeea1cd1467b2dc42abcf',
        'verus/context_producer_acquire_bodies_v1.rs': '896f8394e052cff5b56e2be3faac36dfdd5250a6eeeba43ddd7270cf75dc8745',
    }.items():
        need(hashlib.sha256((tree / SRC.parent / relative).read_bytes()).hexdigest() == pin,
             'reviewed receiver/helper adapter: ' + relative)
    path = SRC.parent / 'verus/context_producer_read_lifecycle_v1.rs'
    historical = (tree / path).read_text()
    need(historical == original(path), 'unchanged historical admission provenance')
    need((tree / SRC.parent / 'verus/context_producer_acquire_historical_bodies_v1.rs').read_text()
         == historical_projection(historical), 'exact historical declaration projection')


def cases():
    return [(k, max(k + 2, 1024), shape, fault)
            for k in (2, 8, 64, 512, 4096) for shape in ('grouped', 'distinct')
            for fault in ('none', 'device', 'count', 'free', 'output', 'epoch', 'alias')] + [
                (8, 65536, 'grouped', 'none'), (8, 65536, 'distinct', 'none')]


def accesses(k, fault):
    return 0 if fault in ('output', 'epoch') else 3 * k - (1 if fault == 'device' else 0)


def parse(text):
    rows = []
    prefix = 'test ' + TEST + ' ... '
    for line in text.splitlines():
        if line.startswith(prefix):
            line = line[len(prefix):]
        if 'producer_acquire,' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 10 and parts[0] == 'producer_acquire', 'benchmark row shape')
        for index in (1, 2, 5, 7, 8, 9):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((int(parts[1]), int(parts[2]), parts[3], parts[4], int(parts[5]), parts[6],
                     int(parts[7]), int(parts[8]), int(parts[9])))
    expected = [(k, capacity, shape, fault, round_, variant, min(8192, max(64, 131072 // k)), accesses(k, fault))
                for k, capacity, shape, fault in cases() for round_ in range(7) for turn in range(2)
                for variant in ['shared' if (round_ + turn) % 2 == 0 else 'baseline']]
    need(len(rows) == len(expected) == 1008, 'complete benchmark row roster')
    grouped = {}
    for row, wanted in zip(rows, expected):
        k, capacity, shape, fault, round_, variant, iterations, elapsed, lookups = row
        need((*row[:7], lookups) == wanted, 'exact ordered benchmark fields')
        need(elapsed > 0, 'positive elapsed time')
        grouped.setdefault((k, capacity, shape, fault), {}).setdefault(variant, []).append(elapsed / iterations)
    return grouped


def table(text):
    lines = ['| Roster | Capacity | Shape | Fault | Shared ns | Frozen ns | Shared/frozen |',
             '| ---: | ---: | --- | --- | ---: | ---: | ---: |']
    for key, rows in parse(text).items():
        shared, baseline = (statistics.median(rows[name]) for name in ('shared', 'baseline'))
        lines.append('| %d | %d | %s | %s | %.2f | %.2f | %.3f |' % (*key, shared, baseline, shared / baseline))
    return '\n'.join(lines)


def selftest(text):
    lines = text.splitlines()
    indices = [i for i, line in enumerate(lines) if 'producer_acquire,' in line]
    first = indices[0]
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, '02'), (2, '1'), (3, 'wrong'), (4, 'wrong'),
                         (5, '1'), (6, 'baseline'), (7, '1'), (8, '0'), (9, '0')):
        changed = list(lines)
        prefix, row = changed[first].split('producer_acquire,', 1)
        parts = ('producer_acquire,' + row).split(',')
        parts[field] = value
        changed[first] = prefix + ','.join(parts)
        variants.append(changed)
    changed = list(lines)
    changed[first], changed[indices[1]] = changed[indices[1]], changed[first]
    variants.append(changed)
    for fault, variant, incorrect in [('device', 'baseline', '6'), ('count', 'shared', '5'),
                                      ('output', 'baseline', '6'), ('epoch', 'shared', '6'),
                                      ('alias', 'baseline', '0')]:
        changed = list(lines)
        index = next(i for i in indices if ',grouped,' + fault + ',' in lines[i] and ',' + variant + ',' in lines[i])
        prefix, row = changed[index].split('producer_acquire,', 1)
        parts = ('producer_acquire,' + row).split(',')
        parts[9] = incorrect
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
