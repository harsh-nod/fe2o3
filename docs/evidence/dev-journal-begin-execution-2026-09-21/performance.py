#!/usr/bin/env python3
"""Authenticate the frozen baseline and replay the complete matched CPU table."""
import hashlib
from pathlib import Path
import re
import statistics
import subprocess

BASELINE = '535763f018e1bf2236d4e8abe7790daf3c9df242'
JOURNAL = Path('crates/fe2o3-runtime-model/src/context_version_journal.rs')
FROZEN = JOURNAL.with_suffix('') / 'begin_tests/benchmark_baseline.rs'
ORIGINAL_SHA = '0a211c6e87d0a24097a01f9f8dc07f7ed8a6ac9070612c75f0f663c563b12a38'
TEST = 'context_version_journal::tests::membership::begin_shared::performance::shared_begin_performance'


def need(value, message):
    if not value:
        raise ValueError(message)


def authenticate(repo, tree):
    original = subprocess.check_output(['git', '-C', str(repo), 'show', BASELINE + ':' + str(JOURNAL)]).decode('ascii')
    start = original.index('    fn preflight_begin_write(')
    end = original.index('    fn read_allocation(', start)
    methods = original[start:end]
    need(hashlib.sha256(methods.encode('ascii')).hexdigest() == ORIGINAL_SHA, 'frozen original method hash')
    expected = (
        '// Frozen from ' + BASELINE + '; only method names/visibility change.\n'
        '// Original two-method bytes SHA256: ' + ORIGINAL_SHA + '\n'
        'use super::*;\n\nimpl ContextVersionJournalV1 {\n'
        + methods.replace('preflight_begin_write(', 'baseline_preflight_begin_write_v1(')
                 .replace('pub fn begin_write(', 'pub(super) fn baseline_begin_write_v1(').rstrip()
        + '\n}\n'
    )
    need((tree / FROZEN).read_text() == expected, 'frozen baseline exact Git correspondence')
    current = (tree / JOURNAL).read_text()
    # These method boundaries are authenticated against the fixed original.
    for name in ('lookup_reserved', 'read_slot', 'store_slot', 'read_allocation',
                 'exact_allocation', 'free_member_slot', 'store_plan', 'count_indexed_access'):
        anchor = ('    pub fn ' if name == 'lookup_reserved' else '    fn ') + name + '('
        def method(text):
            need(text.count(anchor) == 1, 'unique baseline helper')
            offset = text.index(anchor)
            return text[offset:text.index('\n    }\n', offset) + len('\n    }\n')]
        need(method(current) == method(original), 'unchanged frozen helper: ' + name)


def cases():
    result = [(0, 1024, 'normal'), (0, 1024, 'dirty_tail')]
    for count in (1, 8, 64, 512, 4096):
        for pattern in ('normal', 'permuted', 'alias', 'dirty_tail',
                        'device_first', 'device_last', 'member_last', 'scratch_last'):
            result.append((count, max(2 * count, 1024), pattern))
    return result + [(8, 65536, 'normal'), (8, 65536, 'permuted')]


def accesses(count, pattern):
    if pattern == 'device_first':
        return 2
    if pattern == 'device_last':
        return count + 1
    if pattern == 'member_last':
        return 4 * count
    if pattern == 'scratch_last':
        return 4 * count + 1
    return 12 * count + 2


def parse(text):
    rows = []
    for line in text.splitlines():
        if line.startswith('test ' + TEST + ' ... '):
            line = line[len('test ' + TEST + ' ... '):]
        if 'begin_execution' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 9 and parts[0] == 'begin_execution', 'benchmark row shape')
        for index in (1, 2, 4, 6, 7, 8):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((int(parts[1]), int(parts[2]), parts[3], int(parts[4]), parts[5],
                     int(parts[6]), int(parts[7]), int(parts[8])))
    expected = [(k, capacity, pattern, round_, 'shared' if (round_ + turn) % 2 == 0 else 'baseline',
                 min(65536, max(256, 1048576 // max(k, 1))), accesses(k, pattern))
                for k, capacity, pattern in cases() for round_ in range(7) for turn in range(2)]
    need(len(rows) == len(expected) == 616, 'complete benchmark row roster')
    grouped = {}
    for row, wanted in zip(rows, expected):
        k, capacity, pattern, round_, variant, iterations, elapsed, counted = row
        need((k, capacity, pattern, round_, variant, iterations, counted) == wanted, 'exact ordered benchmark fields')
        need(elapsed > 0, 'positive elapsed time')
        grouped.setdefault((k, capacity, pattern), {}).setdefault(variant, []).append(elapsed / iterations)
    return grouped


def table(text):
    groups = parse(text)
    lines = [
        '| Roster | Capacity | Pattern | Shared ns | Frozen ns | Shared/frozen |',
        '| ---: | ---: | --- | ---: | ---: | ---: |',
    ]
    for key, rows in groups.items():
        shared, baseline = (statistics.median(rows[name]) for name in ('shared', 'baseline'))
        lines.append('| %d | %d | %s | %.2f | %.2f | %.3f |' % (*key, shared, baseline, shared / baseline))
    return '\n'.join(lines)


def selftest(text):
    lines = text.splitlines()
    indices = [i for i, line in enumerate(lines) if 'begin_execution,' in line]
    first = indices[0]
    rejected = 0
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, '01'), (2, '0'), (3, 'wrong'), (4, '1'),
                         (5, 'baseline'), (6, '1'), (7, '0'), (8, '0')):
        changed = list(lines)
        prefix, row = changed[first].split('begin_execution,', 1)
        parts = ('begin_execution,' + row).split(',')
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
            rejected += 1
        else:
            raise ValueError('accepted malformed benchmark rows')
    print('PASS: rejected', rejected, 'altered benchmark row sets')
