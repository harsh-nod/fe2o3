#!/usr/bin/env python3
"""Authenticate frozen release and replay instrumented CPU measurements."""
import hashlib
from pathlib import Path
import re
import statistics
import subprocess

BASELINE = '455d3c62a03f1033b5c63e40634592a583e42484'
SRC = Path('crates/fe2o3-runtime-model/src')
ADAPTERS = {
    'lookup_read': 'e1773e500311317e14b45096aac827d45ab56c1dffbc8f3235752d74feabe992',
    'release_reads': '209323f3d810c166dc299a4f0f39a9a2f7fb2e93c085337c416c2f52f1f89c3b',
}
TEST = 'context_read_leases::tests::release_shared::performance::shared_release_performance'


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
         '878c709500a2ac9b7d42a759d8f894c41b04cfe97799d387ef88e4604a90cee7', 'original release methods')
    for name in ADAPTERS:
        methods = methods.replace('pub fn ' + name + '(', 'pub(crate) fn baseline_' + name + '_v1(')
        methods = methods.replace('.' + name + '(', '.baseline_' + name + '_v1(')
    expected = ('// Frozen from ' + BASELINE + '; only method names/visibility change.\n'
                'use super::*;\n\nimpl ContextReadLeasedJournalV1 {\n' + methods.rstrip() + '\n}\n')
    need((tree / SRC / 'context_read_leases/release_baseline.rs').read_text() == expected, 'exact frozen release')
    for anchor in ['impl Deref for ContextReadLeasedJournalV1 {', 'fn read_key(']:
        def block(text):
            start = text.index(anchor)
            return text[start:text.index('\n}\n', start) + 3]
        need(block(source) == block(old), 'unchanged baseline dependency: ' + anchor)
    need(method(source, 'validate_read') == method(old, 'validate_read'), 'unchanged baseline validate_read')
    for relative in ['context_read_leases/declarations.rs', 'context_read_leases/acquire_bodies.rs',
                     'context_read_leases/acquire_declarations.rs', 'context_read_leases/acquire.rs', 'context_version_journal/declarations.rs',
                     'context_version_journal/lookup_bodies.rs', 'context_version_journal/retained.rs',
                     'context_version_journal/retained_bodies.rs']:
        need((tree / SRC / relative).read_text() == original(SRC / relative), 'unchanged baseline dependency: ' + relative)
    path = SRC / 'context_version_journal.rs'
    for name in ['lookup_allocation', 'count_indexed_access']:
        need(method((tree / path).read_text(), name) == method(original(path), name), 'unchanged lookup dependency: ' + name)
    for relative, pin in {
        'src/context_read_leases/release.rs': '49b3b75731a788d172f6b2ee3f8fbdbd6b1bf6e9a1e7729b035f8057b10e155d',
        'src/context_read_leases/release_declarations.rs': '792b2244ddfdbf631958bbcff35ba9b1a2909df0a87be652b70a99f9bcf9aa91',
        'verus/context_stable_release_bodies_v1.rs': 'e7704d0d214151e0a7cfb1f5837494e9f47e33b466ab0111a1bab49039a2f62f',
    }.items():
        need(hashlib.sha256((tree / SRC.parent / relative).read_bytes()).hexdigest() == pin, 'reviewed adapter/declaration: ' + relative)


def cases():
    return [(k, max(k + 2, 1024), shape, fault)
            for k in (1, 8, 64, 512, 4096) for shape in ('grouped', 'distinct')
            for fault in ('none', 'late_reference', 'late_count', 'evidence', 'capacity')
            ] + [(8, 65536, 'grouped', 'none'), (8, 65536, 'distinct', 'none')]


def parse(text):
    rows = []
    prefix = 'test ' + TEST + ' ... '
    for line in text.splitlines():
        if line.startswith(prefix):
            line = line[len(prefix):]
        if 'stable_release,' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 10 and parts[0] == 'stable_release', 'benchmark row shape')
        for index in (1, 2, 5, 7, 8, 9):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((int(parts[1]), int(parts[2]), parts[3], parts[4], int(parts[5]),
                     parts[6], int(parts[7]), int(parts[8]), int(parts[9])))
    expected = [(k, capacity, shape, fault, round_, 'shared' if (round_ + turn) % 2 == 0 else 'baseline',
                 min(8192, max(64, 131072 // k)), 0 if fault in ('evidence', 'capacity') else k - 1 if fault == 'late_reference' else k)
                for k, capacity, shape, fault in cases() for round_ in range(7) for turn in range(2)]
    need(len(rows) == len(expected) == 728, 'complete benchmark row roster')
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
    indices = [i for i, line in enumerate(lines) if 'stable_release,' in line]
    first = indices[0]
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, '01'), (2, '0'), (3, 'wrong'), (4, 'wrong'),
                         (5, '1'), (6, 'baseline'), (7, '1'), (8, '0'), (9, '0')):
        changed = list(lines)
        prefix, row = changed[first].split('stable_release,', 1)
        parts = ('stable_release,' + row).split(',')
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
