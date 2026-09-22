#!/usr/bin/env python3
"""Authenticate frozen acquisition and replay instrumented CPU measurements."""
import hashlib
from pathlib import Path
import re
import statistics
import subprocess

BASELINE = '72bcb1da989ea79c2c48085f5a6c9ba3ce1633f1'
SRC = Path('crates/fe2o3-runtime-model/src')
ADAPTERS = {
    'validate_read_capacity': '95155bea5e4dd21cc09150c22a083ad1f5564aeec5476f53f9853ed7dde83ec1',
    'validate_read': '24d5bd130b3d6d9fa4d8b450bfdfc33de530af957d05e5f78f8a41bfb3c3a3d6',
    'acquire_reads': '2dc83a7b3b8f565d24a6d02bcbfcbc8c03ddb57527e83939c445ce75bbe7f06b',
}
TEST = 'context_read_leases::tests::acquire_shared::performance::shared_acquire_performance'


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
    path = SRC / 'context_read_leases.rs'
    source, old = (tree / path).read_text(), original(path)
    for name, digest in ADAPTERS.items():
        need(hashlib.sha256(method(source, name).encode('ascii')).hexdigest() == digest, 'reviewed public adapter: ' + name)
    methods = '\n'.join(method(old, name) for name in ADAPTERS)
    need(hashlib.sha256(methods.encode('ascii')).hexdigest() ==
         '8eef6ad4a2808440d424361dc0deef43b7ebe3ef7c8cc3570787d0455b23c298', 'original acquisition methods')
    for name in ADAPTERS:
        methods = methods.replace('pub fn ' + name + '(', 'pub(crate) fn baseline_' + name + '_v1(')
        methods = methods.replace('.' + name + '(', '.baseline_' + name + '_v1(')
    methods = methods.replace('fn baseline_validate_read_capacity_v1(&self, count: usize)',
                              'fn baseline_validate_read_capacity_v1(\n        &self,\n        count: usize,\n    )')
    expected = ('// Frozen from ' + BASELINE + '; only method names/visibility change.\n'
                'use super::*;\n\nimpl ContextReadLeasedJournalV1 {\n' + methods.rstrip() + '\n}\n')
    need((tree / SRC / 'context_read_leases/acquire_baseline.rs').read_text() == expected, 'exact frozen acquisition')
    for anchor in ['impl Deref for ContextReadLeasedJournalV1 {', 'fn read_key(']:
        def block(text):
            start = text.index(anchor)
            return text[start:text.index('\n}\n', start) + 3]
        need(block(source) == block(old), 'unchanged baseline dependency: ' + anchor)
    for relative in ['context_read_leases/declarations.rs', 'context_version_journal/declarations.rs',
                     'context_version_journal/lookup_bodies.rs', 'context_version_journal/retained.rs',
                     'context_version_journal/retained_bodies.rs']:
        need((tree / SRC / relative).read_text() == original(SRC / relative), 'unchanged baseline dependency: ' + relative)
    path = SRC / 'context_version_journal.rs'
    for name in ['context_generation', 'lookup_allocation', 'count_indexed_access']:
        need(method((tree / path).read_text(), name) == method(original(path), name), 'unchanged lookup dependency: ' + name)
    need(method((tree / path).read_text(), 'context_generation') ==
         '    pub const fn context_generation(&self) -> u64 {\n        self.context_generation\n    }\n', 'generation getter body')
    for relative, pin in {
        'src/context_read_leases/acquire.rs': 'a7b528937a8fcc31f438ae7715029d68106c19aefeeeb019ab8dffae2920cc77',
        'src/context_read_leases/acquire_declarations.rs': '84c7f08ece64282247d6fc8018b23b968c1058ead8b4c65f127ee3a24da0830f',
        'verus/context_stable_acquire_bodies_v1.rs': '9df1f4b387aac36be6ef0657dd588192f77e52cd5b19876dddbd9abbc2be0e5c',
    }.items():
        need(hashlib.sha256((tree / SRC.parent / relative).read_bytes()).hexdigest() == pin, 'reviewed adapter/declaration: ' + relative)


def cases():
    return [(k, max(k + 2, 1024), shape, fault)
            for k in (1, 8, 64, 512, 4096) for shape in ('grouped', 'distinct')
            for fault in ('none', 'late_extent', 'late_output', 'late_slot')
            ] + [(8, 65536, 'grouped', 'none'), (8, 65536, 'distinct', 'none')]


def parse(text):
    rows = []
    prefix = 'test ' + TEST + ' ... '
    for line in text.splitlines():
        if line.startswith(prefix):
            line = line[len(prefix):]
        if 'stable_acquire,' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 10 and parts[0] == 'stable_acquire', 'benchmark row shape')
        for index in (1, 2, 5, 7, 8, 9):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((int(parts[1]), int(parts[2]), parts[3], parts[4], int(parts[5]),
                     parts[6], int(parts[7]), int(parts[8]), int(parts[9])))
    expected = [(k, capacity, shape, fault, round_, 'shared' if (round_ + turn) % 2 == 0 else 'baseline',
                 min(8192, max(64, 131072 // k)), 0 if fault == 'late_output' else k)
                for k, capacity, shape, fault in cases() for round_ in range(7) for turn in range(2)]
    need(len(rows) == len(expected) == 588, 'complete benchmark row roster')
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
    indices = [i for i, line in enumerate(lines) if 'stable_acquire,' in line]
    first = indices[0]
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, '01'), (2, '0'), (3, 'wrong'), (4, 'wrong'),
                         (5, '1'), (6, 'baseline'), (7, '1'), (8, '0'), (9, '0')):
        changed = list(lines)
        prefix, row = changed[first].split('stable_acquire,', 1)
        parts = ('stable_acquire,' + row).split(',')
        parts[field] = value
        changed[first] = prefix + ','.join(parts)
        variants.append(changed)
    changed = list(lines)
    changed[first], changed[indices[1]] = changed[indices[1]], changed[first]
    variants.append(changed)
    for changed in variants:
        try:
            parse('\n'.join(changed))
        except ValueError:
            pass
        else:
            raise ValueError('accepted malformed benchmark rows')
    print('PASS: rejected', len(variants), 'altered benchmark row sets')
