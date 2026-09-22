#!/usr/bin/env python3
"""Authenticate frozen producer release and replay instrumented CPU measurements."""
import hashlib
from pathlib import Path
import re
import statistics
import subprocess

BASELINE = 'db5dd95e182ca7c90a00e3e98d234710c90c6264'
SRC = Path('crates/fe2o3-runtime-model/src')
TEST = 'context_producer_reads::tests::release_shared::performance::shared_producer_release_performance'


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
        ("producer_arena_remove_v1", "producer_arena_remove_v1"),
        ("producer_release_header_v1", "producer_release_preflight_exec_v1"),
        ("producer_released_requests_v1", "producer_release_commit_exec_v1"),
        ("producer_release_scan_ready_v1", "producer_release_preflight_ready_v1"),
        ("producer_released_counts_v1", "producer_release_arena_prefix_v1"),
        ("producer_release_execution_relation_v1", "producer_release_execution_relation_v1"),
        ("producer_release_preserves_v1", "producer_release_preserves_v1"),
        ("producer_release_contents_exec_v1", "producer_release_contents_exec_v1"),
        ("producer_release_exec_v1", "producer_release_exec_v1"),
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
    need(count == 23, 'exact historical declaration roster')
    return ('// Exact release declaration blocks from the historical lifecycle, in the importing type universe.\n'
            'use super::*;\n\nverus! {\n\n' + ''.join(pieces).rstrip() + '\n\n}\n')


def authenticate(repo, tree):
    def original(path):
        return subprocess.check_output(['git', '-C', str(repo), 'show', BASELINE + ':' + str(path)]).decode('ascii')

    path = SRC / 'context_producer_reads.rs'
    old = original(path)
    frozen = method(old, 'release_producer_reads').replace(
        'pub fn release_producer_reads(', 'pub(crate) fn baseline_release_producer_reads_v1(')
    expected = ('// Frozen from ' + BASELINE + '; only method name/visibility changes.\n'
                'use super::*;\n\nimpl ContextProducerReadJournalV1 {\n' + frozen.rstrip() + '\n}\n')
    need((tree / SRC / 'context_producer_reads/release_baseline.rs').read_text() == expected,
         'exact frozen release method')
    current = (tree / path).read_text()
    for name in ['lookup_producer_read', 'inspect_producer_read', 'producer_read_status', 'status',
                 'validate_producer_read', 'acquire_producer_reads']:
        need(method(current, name) == method(old, name), 'unchanged release dependency adapter: ' + name)

    def read_key(source):
        start = source.index('fn read_key(')
        return source[start:source.index('\n}\n', start) + 3]
    need(read_key(current) == read_key(old), 'unchanged frozen ordering helper')
    need('#[cfg(test)]\nfn read_key(' in current and '#[cfg(test)]\nmod release_baseline;' in current,
         'test-only frozen helper and baseline')
    for relative in [
        'context_read_leases.rs', 'context_read_leases/declarations.rs',
        'context_producer_reads/declarations.rs', 'context_producer_reads/query.rs',
        'context_producer_reads/query_bodies.rs', 'context_producer_reads/acquire.rs',
        'context_producer_reads/acquire_bodies.rs', 'context_producer_reads/acquire_declarations.rs',
        'context_version_journal.rs', 'context_version_journal/declarations.rs',
        'context_version_journal/lookup_bodies.rs', 'context_version_journal/retained.rs',
        'context_version_journal/retained_bodies.rs', 'context_version_journal/writer_lookup_bodies.rs',
    ]:
        need((tree / SRC / relative).read_text() == original(SRC / relative),
             'unchanged frozen dependency: ' + relative)
    for relative, pin in {
        'src/context_producer_reads.rs': '692cc3e92e96ac4e6cfd408360b0b172c19a8fe9ca0d82a56470300b5ad2e331',
        'src/context_producer_reads/release.rs': '112ccfb9d7cc9214ac06e6cb10beaad9ee3230fa32fcd3986791d3b432304491',
        'src/context_producer_reads/release_declarations.rs': '64e265db83f375d8e44aa99170ff7351b75a116d595fcafcc0dade4e5997186b',
        'verus/context_producer_release_bodies_v1.rs': '51551cbb72d25f5a37f03d3600c9ca7b55104094dbf1ee1c267a4650032213d5',
    }.items():
        need(hashlib.sha256((tree / SRC.parent / relative).read_bytes()).hexdigest() == pin,
             'reviewed public/private capacity observation adapter: ' + relative)
    path = SRC.parent / 'verus/context_producer_read_lifecycle_v1.rs'
    historical = (tree / path).read_text()
    need(historical == original(path), 'unchanged historical release provenance')
    need((tree / SRC.parent / 'verus/context_producer_release_historical_bodies_v1.rs').read_text()
         == historical_projection(historical), 'exact historical declaration projection')


def cases():
    result = [(k, max(k + 2, 1024), shape, phase, 'none')
              for k in (1, 8, 64, 512, 4096) for shape in ('grouped', 'distinct')
              for phase in ('pending', 'unknown', 'success', 'no_effect')]
    for shape in ('grouped', 'distinct'):
        for phase in ('pending', 'unknown', 'success', 'no_effect'):
            result.extend((8, 1024, shape, phase, fault)
                          for fault in ('reference', 'count', 'evidence', 'capacity'))
            result.append((8, 65536, shape, phase, 'none'))
    return result


def accesses(k, phase, fault):
    weight = 3 if phase in ('pending', 'unknown') else 1
    return 0 if fault in ('evidence', 'capacity') else weight * (k - (fault == 'reference'))


def parse(text):
    rows = []
    prefix = 'test ' + TEST + ' ... '
    for line in text.splitlines():
        if line.startswith(prefix):
            line = line[len(prefix):]
        if 'producer_release,' not in line:
            continue
        parts = line.split(',')
        need(len(parts) == 11 and parts[0] == 'producer_release', 'benchmark row shape')
        for index in (1, 2, 6, 8, 9, 10):
            need(re.fullmatch(r'0|[1-9][0-9]*', parts[index]) is not None, 'benchmark integer field')
        rows.append((int(parts[1]), int(parts[2]), parts[3], parts[4], parts[5], int(parts[6]),
                     parts[7], int(parts[8]), int(parts[9]), int(parts[10])))
    expected = [(k, capacity, shape, phase, fault, round_, variant,
                 min(8192, max(64, 131072 // k)), accesses(k, phase, fault))
                for k, capacity, shape, phase, fault in cases() for round_ in range(7) for turn in range(2)
                for variant in ['shared' if (round_ + turn) % 2 == 0 else 'baseline']]
    need(len(rows) == len(expected) == 1120, 'complete benchmark row roster')
    grouped = {}
    for row, wanted in zip(rows, expected):
        k, capacity, shape, phase, fault, round_, variant, iterations, elapsed, lookups = row
        need((*row[:8], lookups) == wanted, 'exact ordered benchmark fields')
        need(elapsed > 0, 'positive elapsed time')
        grouped.setdefault((k, capacity, shape, phase, fault), {}).setdefault(variant, []).append(elapsed / iterations)
    return grouped


def table(text):
    lines = ['| Roster | Capacity | Shape | State | Fault | Shared ns | Frozen ns | Shared/frozen |',
             '| ---: | ---: | --- | --- | --- | ---: | ---: | ---: |']
    for key, rows in parse(text).items():
        shared, baseline = (statistics.median(rows[name]) for name in ('shared', 'baseline'))
        lines.append('| %d | %d | %s | %s | %s | %.2f | %.2f | %.3f |' % (*key, shared, baseline, shared / baseline))
    return '\n'.join(lines)


def selftest(text):
    lines = text.splitlines()
    indices = [i for i, line in enumerate(lines) if 'producer_release,' in line]
    first = indices[0]
    variants = [lines[:first] + lines[first + 1:], lines[:first] + [lines[first]] + lines[first:]]
    for field, value in ((0, 'wrong'), (1, '01'), (2, '1'), (3, 'wrong'), (4, 'wrong'),
                         (5, 'wrong'), (6, '1'), (7, 'baseline'), (8, '1'), (9, '0'), (10, '0')):
        changed = list(lines)
        prefix, row = changed[first].split('producer_release,', 1)
        parts = ('producer_release,' + row).split(',')
        parts[field] = value
        changed[first] = prefix + ','.join(parts)
        variants.append(changed)
    changed = list(lines)
    changed[first], changed[indices[1]] = changed[indices[1]], changed[first]
    variants.append(changed)
    for phase, fault, variant in [('pending', 'reference', 'baseline'), ('unknown', 'count', 'shared'),
                                  ('success', 'evidence', 'baseline'), ('no_effect', 'capacity', 'shared'),
                                  ('success', 'none', 'baseline')]:
        changed = list(lines)
        index = next(i for i in indices if ',grouped,' + phase + ',' + fault + ',' in lines[i]
                     and ',' + variant + ',' in lines[i])
        prefix, row = changed[index].split('producer_release,', 1)
        parts = ('producer_release,' + row).split(',')
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
