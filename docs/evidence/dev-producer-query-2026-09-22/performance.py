#!/usr/bin/env python3
"""Authenticate frozen producer queries and replay instrumented CPU measurements."""
import hashlib
from pathlib import Path
import re
import statistics
import subprocess

BASELINE = '9d2ec2b98819daf14b2a0bc72681f95a3cc571a9'
SRC = Path('crates/fe2o3-runtime-model/src')
TEST = 'context_producer_reads::tests::query_shared::performance::shared_producer_query_performance'


def need(value, message):
    if not value:
        raise ValueError(message)


def method(source, name):
    anchors = [prefix + name + '(' for prefix in ('    pub fn ', '    pub const fn ', '    fn ')]
    anchors = [anchor for anchor in anchors if anchor in source]
    need(len(anchors) == 1 and source.count(anchors[0]) == 1, 'unique source method: ' + name)
    start = source.index(anchors[0])
    return source[start:source.index('\n    }\n', start) + len('\n    }\n')]


def authenticate(repo, tree):
    def original(path):
        return subprocess.check_output(['git', '-C', str(repo), 'show', BASELINE + ':' + str(path)]).decode('ascii')
    owners = [
        ('context_producer_reads', 'ContextProducerReadJournalV1',
         ['status', 'validate_producer_read', 'lookup_producer_read', 'producer_read_status']),
        ('context_version_journal', 'ContextVersionJournalV1', ['lookup_writer']),
    ]
    for owner, typename, names in owners:
        path = SRC / (owner + '.rs')
        old = original(path)
        methods = '\n'.join(method(old, name) for name in names)
        for name in names:
            methods = methods.replace('    pub fn ' + name + '(', '    pub(crate) fn baseline_' + name + '_v1(')
            methods = methods.replace('    fn ' + name + '(', '    pub(crate) fn baseline_' + name + '_v1(')
            methods = methods.replace('.' + name + '(', '.baseline_' + name + '_v1(')
        if owner == 'context_producer_reads':
            methods = methods.replace('.lookup_writer(', '.baseline_lookup_writer_v1(')
        expected = ('// Frozen from ' + BASELINE + '; only method names/visibility change.\n'
                    'use super::*;\n\nimpl ' + typename + ' {\n' + methods.rstrip() + '\n}\n')
        need((tree / SRC / owner / 'query_baseline.rs').read_text() == expected, 'exact frozen queries: ' + owner)
    for relative, anchor in [
        ('context_producer_reads.rs', 'impl Deref for ContextProducerReadJournalV1 {'),
        ('context_read_leases.rs', 'impl Deref for ContextReadLeasedJournalV1 {'),
    ]:
        def block(text):
            start = text.index(anchor)
            return text[start:text.index('\n}\n', start) + 3]
        need(block((tree / SRC / relative).read_text()) == block(original(SRC / relative)), 'unchanged receiver projection')
    for relative in ['context_read_leases.rs', 'context_read_leases/declarations.rs',
                     'context_producer_reads/declarations.rs', 'context_version_journal/declarations.rs',
                     'context_version_journal/lookup_bodies.rs', 'context_version_journal/retained.rs',
                     'context_version_journal/retained_bodies.rs']:
        need((tree / SRC / relative).read_text() == original(SRC / relative), 'unchanged baseline dependency: ' + relative)
    path = SRC / 'context_version_journal.rs'
    for name in ['lookup_allocation', 'context_generation', 'read_slot', 'count_indexed_access']:
        need(method((tree / path).read_text(), name) == method(original(path), name), 'unchanged lookup dependency: ' + name)
    for relative, pin in {
        'src/context_producer_reads.rs': '237a3f5183743c554c9116983eaf3085f98e80ad1fc73c2b8685e6adcdb3c829',
        'src/context_version_journal.rs': '9613c065ef8e724419c907291682885f74e25660b121676c8b1ceebf5cd5cf0e',
        'src/context_producer_reads/query.rs': '44c249dff2fbf3eb5739b5d1d33b00cc1087d4f161799c9efee15d7d0d0691d1',
        'verus/context_producer_query_bodies_v1.rs': 'ce6727ad795cb79537b30c8adac90edbced441cf8d7d5739e255b14d306e629b',
    }.items():
        need(hashlib.sha256((tree / SRC.parent / relative).read_bytes()).hexdigest() == pin, 'reviewed receiver/helper adapter: ' + relative)
    path = SRC.parent / 'verus/context_producer_read_lifecycle_v1.rs'
    historical = (tree / path).read_text()
    need(historical == original(path), 'unchanged historical query provenance')
    start = historical.index('pub fn same_producer_exec_v1(')
    end = historical.index('pub open spec fn producer_capacity_decision_v1(', start)
    expected = ("// Exact query declarations from the historical lifecycle, in the importing root's type universe.\n"
                'use super::*;\n\nverus! {\n\n' + historical[start:end].rstrip() + '\n\n}\n')
    path = SRC.parent / 'verus/context_producer_query_historical_bodies_v1.rs'
    need((tree / path).read_text() == expected, 'exact historical declaration projection')


def cases():
    return [(capacity, scenario, operation)
            for capacity in (8, 65536)
            for scenario in ('pending', 'unknown', 'success', 'no_effect', 'success_reused',
                             'no_effect_reused', 'allocation', 'backlink', 'device', 'writer',
                             'reference', 'pending_lineage', 'resolved_lineage')
            for operation in ('status', 'validate', 'lookup', 'query')]


def accesses(scenario, operation, variant):
    if scenario == 'reference' and operation in ('lookup', 'query'):
        return 0
    count = 3 if scenario in ('pending', 'unknown', 'writer', 'reference') else (
        2 if scenario in ('backlink', 'device', 'pending_lineage') else 1)
    if operation == 'query' and variant == 'baseline' and scenario in (
            'pending', 'unknown', 'success', 'no_effect', 'success_reused', 'no_effect_reused', 'reference'):
        count *= 2
    return count


def parse(text):
    rows = []
    prefix = 'test ' + TEST + ' ... '
    for line in text.splitlines():
        if line.startswith(prefix):
            line = line[len(prefix):]
        if 'producer_query,' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 9 and parts[0] == 'producer_query', 'benchmark row shape')
        for index in (1, 4, 6, 7, 8):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((int(parts[1]), parts[2], parts[3], int(parts[4]), parts[5],
                     int(parts[6]), int(parts[7]), int(parts[8])))
    expected = [(capacity, scenario, operation, round_, variant, 8192, accesses(scenario, operation, variant))
                for capacity, scenario, operation in cases() for round_ in range(7) for turn in range(2)
                for variant in ['shared' if (round_ + turn) % 2 == 0 else 'baseline']]
    need(len(rows) == len(expected) == 1456, 'complete benchmark row roster')
    grouped = {}
    for row, wanted in zip(rows, expected):
        capacity, scenario, operation, round_, variant, iterations, elapsed, lookups = row
        need((*row[:6], lookups) == wanted, 'exact ordered benchmark fields')
        need(elapsed > 0, 'positive elapsed time')
        grouped.setdefault((capacity, scenario, operation), {}).setdefault(variant, []).append(elapsed / iterations)
    return grouped


def table(text):
    lines = ['| Capacity | State/fault | Operation | Shared ns | Frozen ns | Shared/frozen |',
             '| ---: | --- | --- | ---: | ---: | ---: |']
    for key, rows in parse(text).items():
        shared, baseline = (statistics.median(rows[name]) for name in ('shared', 'baseline'))
        lines.append('| %d | %s | %s | %.2f | %.2f | %.3f |' % (*key, shared, baseline, shared / baseline))
    return '\n'.join(lines)


def selftest(text):
    lines = text.splitlines()
    indices = [i for i, line in enumerate(lines) if 'producer_query,' in line]
    first = indices[0]
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, '08'), (2, 'wrong'), (3, 'wrong'), (4, '1'),
                         (5, 'baseline'), (6, '1'), (7, '0'), (8, '0')):
        changed = list(lines)
        prefix, row = changed[first].split('producer_query,', 1)
        parts = ('producer_query,' + row).split(',')
        parts[field] = value
        changed[first] = prefix + ','.join(parts)
        variants.append(changed)
    changed = list(lines)
    changed[first], changed[indices[1]] = changed[indices[1]], changed[first]
    variants.append(changed)
    for operation, scenario, variant, incorrect in [
        ('query', 'pending', 'baseline', '3'),
        ('query', 'pending', 'shared', '6'),
        ('query', 'success', 'baseline', '1'),
        ('query', 'writer', 'baseline', '6'),
        ('query', 'reference', 'baseline', '3'),
    ]:
        changed = list(lines)
        index = next(i for i in indices if ',' + scenario + ',' + operation + ',' in lines[i]
                     and ',' + variant + ',' in lines[i])
        prefix, row = changed[index].split('producer_query,', 1)
        parts = ('producer_query,' + row).split(',')
        parts[8] = incorrect
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

