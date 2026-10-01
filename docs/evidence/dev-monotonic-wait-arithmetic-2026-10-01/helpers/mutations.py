#!/usr/bin/env python3
"""Exact source-only arithmetic mutations; no solver or accepted proof kills."""
import hashlib
import json
from pathlib import Path

SOURCE = Path('/run/shm/fe2o3-wait-arithmetic-source-20261001-attempt-2')
BODY = Path('crates/fe2o3-kfd/src/wait_arithmetic_body.rs')
PROOF = Path('crates/fe2o3-runtime-model/verus/monotonic_wait_arithmetic_v1.rs')
PINS = {
    BODY: '1d4e8a628e42cf8dd7607f6902d55f92dc0df8a4d1e4fc69afd3f6844f006c57',
    PROOF: '43ec6bf0c1e2a3bdd561818502b1b63325d3f0c8d4002e871a8ded80288a008b',
}

# Each replacement is local to an actual shared executable macro arm.
CASES = (
    ('increment-saturation-zero', 'increment', 'u32::MAX\n        } else', '0\n        } else'),
    ('increment-no-progress', 'increment', '$attempts + 1', '$attempts'),
    ('prefix-spin-boundary', 'prefix', '$attempts <= SPIN_ATTEMPTS_V1 {', '$attempts < SPIN_ATTEMPTS_V1 {'),
    ('prefix-yield-boundary', 'prefix', 'SPIN_ATTEMPTS_V1 + YIELD_ATTEMPTS_V1', 'SPIN_ATTEMPTS_V1 + YIELD_ATTEMPTS_V1 - 1'),
    ('prefix-wrong-spin-action', 'prefix', 'WaitPrefixV1::Spin', 'WaitPrefixV1::Yield'),
    ('sleep-reversed-seconds', 'sleep', 'remaining.0 < $next.0', 'remaining.0 > $next.0'),
    ('sleep-reversed-nanos', 'sleep', 'remaining.1 < $next.1', 'remaining.1 > $next.1'),
    ('sleep-wrong-equal-seconds-gate', 'sleep', 'remaining.0 == $next.0', 'remaining.0 != $next.0'),
    ('sleep-absent-remaining-selected', 'sleep', 'None => false', 'None => true'),
    ('backoff-premature-saturation', 'backoff', '$next.0 > u64::MAX / 2', '$next.0 >= u64::MAX / 2'),
    ('backoff-wrong-saturation-nanos', 'backoff', 'WAIT_NANOS_PER_SECOND_V1 - 1', '0'),
    ('backoff-nanos-not-doubled', 'backoff', '$next.1 * 2', '$next.1'),
    ('backoff-lost-carry', 'backoff', '(1_u64, doubled_nanos - WAIT_NANOS_PER_SECOND_V1)', '(0_u64, doubled_nanos - WAIT_NANOS_PER_SECOND_V1)'),
    ('backoff-seconds-not-doubled', 'backoff', '$next.0 * 2 + carry', '$next.0 + carry'),
    ('backoff-reversed-ceiling-seconds', 'backoff', 'doubled.0 < $ceiling.0', 'doubled.0 > $ceiling.0'),
    ('backoff-reversed-ceiling-nanos', 'backoff', 'doubled.1 < $ceiling.1', 'doubled.1 > $ceiling.1'),
    ('backoff-ceiling-not-applied', 'backoff', '} else {\n            $ceiling\n        }', '} else {\n            doubled\n        }'),
)
EQUIVALENTS = (
    ('equivalent-backoff-equal-pair-selection', 'backoff', 'doubled.1 < $ceiling.1', 'doubled.1 <= $ceiling.1'),
)
BOUNDARIES = {
    'increment': 'increment_wait_attempts_v1',
    'prefix': 'wait_prefix_v1',
    'sleep': 'wait_sleep_uses_remaining_v1',
    'backoff': 'wait_backoff_pair_v1',
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def checked_sources(sources):
    need(set(sources) == set(PINS), 'exact complete two-input proof closure')
    need(all(isinstance(data, bytes) and sha(data) == PINS[path]
             for path, data in sources.items()), 'unchanged frozen shared body and contracts')
    return sources


def sources():
    result = {}
    for relative in PINS:
        path = SOURCE / relative
        need(path.resolve() == path and path.is_file() and not path.is_symlink(), 'canonical original source')
        result[relative] = path.read_bytes()
    return checked_sources(result)


def arm_span(text, arm):
    need(arm in BOUNDARIES, 'known executable macro arm')
    marker = f'    ({arm}, '
    need(text.count(marker) == 1, 'one exact executable arm')
    start = text.index(marker)
    body_start = text.index('=> {{\n', start) + len('=> {{\n')
    end = text.index('\n    }};', body_start)
    return body_start, end


def project(original, case):
    checked_sources(original)
    name, arm, before, after = case
    need(case in CASES or case in EQUIVALENTS, 'only exact reviewed mutation tuples')
    text = original[BODY].decode('ascii')
    start, end = arm_span(text, arm)
    body = text[start:end]
    need(before != after and body.count(before) == 1, 'one non-vacuous exact body replacement')
    changed = text[:start] + body.replace(before, after, 1) + text[end:]
    changed_start, changed_end = arm_span(changed, arm)
    need(changed_start == start and changed[:start] == text[:start]
         and changed[changed_end:] == text[end:], 'all declarations and other macro arms unchanged')
    files = dict(original)
    files[BODY] = changed.encode('ascii')
    need(files[PROOF] == original[PROOF] and files[BODY] != original[BODY], 'contracts fixed; actual body changed')
    equivalent = case in EQUIVALENTS
    row = {'name': name, 'arm': arm, 'expected_boundary': BOUNDARIES[arm],
           'path': str(BODY), 'root': str(PROOF), 'proof_inputs': 2,
           'before_sha256': sha(before.encode()), 'after_sha256': sha(after.encode()),
           'body_sha256': sha(files[BODY]), 'contract_sha256': sha(files[PROOF]),
           'full_root_only': True, 'selector': None, 'qualified_kill': False,
           'result_equivalent_control': equivalent,
           'boundary': 'numeric-result-only; no native clock, Duration adapter or scheduler guarantee'}
    return row, files


def construct():
    original = sources()
    rows, equivalents, projections = {}, {}, {}
    for case in (*CASES, *EQUIVALENTS):
        row, files = project(original, case)
        need(row['name'] not in projections, 'distinct case name')
        projections[row['name']] = files
        (equivalents if row['result_equivalent_control'] else rows)[row['name']] = row
    need(len(rows) == 17 and len(equivalents) == 1, 'exact17 negatives plus one equivalent control')
    need(len({row['body_sha256'] for row in (*rows.values(), *equivalents.values())}) == 18,
         'distinct actual-body projections')
    need(sources() == original, 'frozen source unchanged after construction')
    return rows, equivalents, projections


if __name__ == '__main__':
    rows, equivalents, _ = construct()
    roster = {'negatives': rows, 'equivalents': equivalents}
    print(json.dumps({'source_only': True, 'negative_candidates': len(rows),
                      'equivalent_controls': len(equivalents), 'proof_inputs': 2,
                      'solver_runs': 0, 'qualified_kills': 0,
                      'roster_sha256': sha(json.dumps(roster, sort_keys=True, separators=(',', ':')).encode())}))
