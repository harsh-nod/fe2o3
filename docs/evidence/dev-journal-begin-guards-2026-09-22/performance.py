#!/usr/bin/env python3
"""Authenticate frozen owners and replay instrumented CPU regression measurements."""
import hashlib
from pathlib import Path
import re
import statistics
import subprocess

BASELINE = 'e8134e770a30d12aa4abd190bd29c5083ea5b3b5'
SRC = Path('crates/fe2o3-runtime-model/src')
ADAPTERS = {
    "context_version_journal": {
        "lookup_allocation": "4f4b918168ebd356990c82c9382c25abc894b41ff4e2d1fe43fc80582dadecfa"
    },
    "context_read_leases": {
        "reader_count": "8504f111d1826648f41d7726a366d81f64e420638d170b4b12898d4538de0704",
        "require_unread_writes": "bf2df4831778e7e3b37d59d7e962df81db0f14c700f29f153285d869a079190d",
        "begin_write": "20308ef978fc694095f3ede8d320dc1be4e6d8cffe91c34600d4a1e5c3fb83e7"
    },
    "context_producer_reads": {
        "reader_count": "285ead4384ebf987f22bb3917348d41e425603d263a30abbcb105ae251633c6f",
        "require_unread_writes": "bf2df4831778e7e3b37d59d7e962df81db0f14c700f29f153285d869a079190d",
        "begin_write": "7a5217d11e393f59f02092f7eac6e06eed26c18bfa9f7abac8bb6035798f0ba7"
    }
}
OWNERS = [
    ('context_version_journal', 'ContextVersionJournalV1', ['lookup_allocation'],
     '7fb57f610aad19a1d12482d46e35eb4a2644d75db5fc792ccee896bb97ee3ebf'),
    ('context_read_leases', 'ContextReadLeasedJournalV1', ['reader_count', 'require_unread', 'begin_write'],
     '1a0fb4cbe9a41d9a692bd29739e464881e13bc03331f666dc1262a46cae092c4'),
    ('context_producer_reads', 'ContextProducerReadJournalV1', ['reader_count', 'require_unread', 'begin_write'],
     'd0d108130b953db0e2555847b31ab5c269da8d8ba72a8665f131b89ebb73a65e'),
]
TESTS = ['context_producer_reads::tests::begin_guards::shared_guards_performance',
         'context_read_leases::tests::begin_guards::shared_guards_performance']


def need(value, message):
    if not value:
        raise ValueError(message)


def method(source, name):
    anchors = ['    pub fn ' + name + '(', '    fn ' + name + '(']
    anchors = [anchor for anchor in anchors if anchor in source]
    need(len(anchors) == 1 and source.count(anchors[0]) == 1, 'unique source method: ' + name)
    start = source.index(anchors[0])
    return source[start:source.index('\n    }\n', start) + len('\n    }\n')]


def authenticate(repo, tree):
    for module, methods in ADAPTERS.items():
        source = (tree / SRC / (module + '.rs')).read_text()
        for name, digest in methods.items():
            need(hashlib.sha256(method(source, name).encode('ascii')).hexdigest() == digest,
                 'reviewed shared-body adapter: ' + module + '::' + name)
    def original(path):
        return subprocess.check_output(['git', '-C', str(repo), 'show', BASELINE + ':' + str(path)]).decode('ascii')
    for module, owner, names, digest in OWNERS:
        path = SRC / (module + '.rs')
        old = original(path)
        methods = '\n'.join(method(old, name) for name in names)
        need(hashlib.sha256(methods.encode('ascii')).hexdigest() == digest, 'original methods identity')
        for name in names:
            methods = methods.replace('pub fn ' + name + '(', 'pub(crate) fn baseline_' + name + '_v1(')
            methods = methods.replace('    fn ' + name + '(', '    pub(crate) fn baseline_' + name + '_v1(')
        methods = methods.replace('.reader_count(', '.baseline_reader_count_v1(')
        methods = methods.replace('.require_unread(', '.baseline_require_unread_v1(')
        methods = methods.replace('.journal.lookup_allocation(', '.journal.baseline_lookup_allocation_v1(')
        methods = methods.replace('.stable.begin_write(', '.stable.baseline_begin_write_v1(')
        expected = ('// Frozen from ' + BASELINE + '; only method names/visibility change.\n'
                    'use super::*;\n\nimpl ' + owner + ' {\n' + methods.rstrip() + '\n}\n')
        need((tree / SRC / module / 'guard_baseline.rs').read_text() == expected, 'exact frozen baseline: ' + module)
        if module != 'context_version_journal':
            start = old.index('#[derive(Clone, Copy, Debug, Eq, PartialEq)]')
            block = old[start:old.index('\nimpl Deref for ', start)].rstrip() + '\n'
            macro, pin = {
                'context_read_leases': ('context_read_declarations_v1', '4ad3645a38a020cae195c21f37cd622066cdfc41e0de581be4652e9fa065ce06'),
                'context_producer_reads': ('context_producer_read_declarations_v1', 'eb9b90a1b9a2f1a115f0dea83c77cd0907da2aad0e8ce18c3f0fbb9511745eb9'),
            }[module]
            need(hashlib.sha256(block.encode('ascii')).hexdigest() == pin, 'original owner declaration identity')
            expected = ('// The same owner and value declaration tokens are compiled by Rust and Verus.\n'
                        + macro + '! {\n' + block + '}\n')
            need((tree / SRC / module / 'declarations.rs').read_text() == expected, 'exact private owner declarations')
    for file in ['declarations.rs', 'begin.rs', 'begin_bodies.rs', 'retained.rs', 'retained_bodies.rs']:
        path = SRC / 'context_version_journal' / file
        need((tree / path).read_text() == original(path), 'unchanged raw Begin dependency: ' + file)
    path = SRC / 'context_version_journal.rs'
    for name in ['begin_write', 'read_allocation', 'exact_allocation', 'count_indexed_access']:
        need(method((tree / path).read_text(), name) == method(original(path), name), 'unchanged journal helper: ' + name)


def cases():
    result = [(0, 1024, 'normal'), (0, 1024, 'invalid_writer')]
    for count in (1, 8, 64, 512, 4096):
        for pattern in ('normal', 'invalid_first', 'invalid_last', 'busy_first', 'busy_last',
                        'pending_last', 'unrelated', 'invalid_writer'):
            result.append((count, max(2 * count, 1024), pattern))
    return result + [(8, 65536, 'normal'), (8, 65536, 'unrelated')]


def accesses(owner, scope, count, pattern):
    if pattern in ('invalid_first', 'busy_first'):
        return 1
    if pattern in ('invalid_last', 'busy_last'):
        return count
    if scope == 'guard':
        return count + int(pattern == 'pending_last')
    if pattern == 'pending_last':
        return (2 if owner == 'stable' else 3) * (count + 1)
    if pattern == 'invalid_writer':
        return (1 if owner == 'stable' else 2) * count + 1
    return (13 if owner == 'stable' else 14) * count + 2


def parse(text):
    rows = []
    for line in text.splitlines():
        for test in TESTS:
            prefix = 'test ' + test + ' ... '
            if line.startswith(prefix):
                line = line[len(prefix):]
        if 'begin_guards,' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 11 and parts[0] == 'begin_guards', 'benchmark row shape')
        for index in (3, 4, 6, 8, 9, 10):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((parts[1], parts[2], int(parts[3]), int(parts[4]), parts[5], int(parts[6]),
                     parts[7], int(parts[8]), int(parts[9]), int(parts[10])))
    expected = [(owner, scope, k, capacity, pattern, round_, 'shared' if (round_ + turn) % 2 == 0 else 'baseline',
                 min(16384, max(128, 262144 // max(k, 1))), accesses(owner, scope, k, pattern))
                for owner in ('producer', 'stable') for k, capacity, pattern in cases()
                for scope in ('guard', 'begin') for round_ in range(7) for turn in range(2)]
    need(len(rows) == len(expected) == 2464, 'complete benchmark row roster')
    grouped = {}
    for row, wanted in zip(rows, expected):
        owner, scope, k, capacity, pattern, round_, variant, iterations, elapsed, counted = row
        need((*row[:8], counted) == wanted, 'exact ordered benchmark fields')
        need(elapsed > 0, 'positive elapsed time')
        grouped.setdefault((owner, scope, k, capacity, pattern), {}).setdefault(variant, []).append(elapsed / iterations)
    return grouped


def table(text):
    groups = parse(text)
    lines = ['| Owner | Scope | Roster | Capacity | Pattern | Shared ns | Frozen ns | Shared/frozen |',
             '| --- | --- | ---: | ---: | --- | ---: | ---: | ---: |']
    for key, rows in groups.items():
        shared, baseline = (statistics.median(rows[name]) for name in ('shared', 'baseline'))
        lines.append('| %s | %s | %d | %d | %s | %.2f | %.2f | %.3f |' % (*key, shared, baseline, shared / baseline))
    return '\n'.join(lines)


def selftest(text):
    lines = text.splitlines()
    indices = [i for i, line in enumerate(lines) if 'begin_guards,' in line]
    first = indices[0]
    rejected = 0
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, 'stable'), (2, 'begin'), (3, '01'), (4, '0'), (5, 'wrong'),
                         (6, '1'), (7, 'baseline'), (8, '1'), (9, '0'), (10, '1')):
        changed = list(lines)
        prefix, row = changed[first].split('begin_guards,', 1)
        parts = ('begin_guards,' + row).split(',')
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
