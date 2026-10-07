#!/usr/bin/env python3
"""Closed whole-root logical diagnostics; timeout/front-end refusals never count."""
import hashlib
import json
from pathlib import Path

VERUS = {'profile': 'release', 'version': '0.2026.08.09.92f466f',
         'platform': {'os': 'linux', 'arch': 'x86_64'},
         'toolchain': '1.97.1-x86_64-unknown-linux-gnu',
         'commit': '92f466f247f45128c630d1c843fd6e27d2115587'}
LOGICAL = {'assertion failed', 'postcondition not satisfied',
           'invariant not satisfied at end of loop body'}
LIMIT = 4 * 1024 * 1024


def need(value, message):
    if not value:
        raise ValueError(message)


def parse(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate diagnostic key')
            result[key] = value
        return result
    need(type(raw) is str and len(raw.encode()) <= LIMIT, 'bounded UTF-8 diagnostic')
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError('non-finite JSON')))


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                      allow_nan=False).encode()).hexdigest()


def normalized_diagnostics(raw, root, files):
    root = root.resolve()

    def visit(value):
        if isinstance(value, list):
            return [visit(item) for item in value]
        if not isinstance(value, dict):
            return value
        result = {}
        for key, item in value.items():
            if key == 'rendered':
                need(type(item) is str, 'text-only human rendering')
            elif key == 'file_name':
                need(type(item) is str and Path(item).is_absolute(), 'absolute source diagnostic')
                path = Path(item).resolve().relative_to(root)
                need(path in files, 'diagnostic belongs to exact compiled input')
                result[key] = str(path)
            else:
                result[key] = visit(item)
        return result

    need(type(raw) is str and len(raw.encode()) <= LIMIT, 'bounded stderr')
    rows = [parse(line) for line in raw.splitlines() if line.strip()]
    for row in rows:
        need(type(row) is dict and row.get('$message_type') == 'diagnostic'
             and row.get('code') is None and row.get('level') in ('error', 'note'),
             'only calibrated logical diagnostics')
    return visit(rows)


def classify(status, stdout, stderr, root, files, expected, negative):
    try:
        need(type(status) is int and status == (1 if negative else 0), 'exact logical exit')
        report = parse(stdout)
        need(type(report) is dict and set(report) == {'func-details', 'verification-results', 'verus'},
             'complete whole-root report schema')
        need(report['verus'] == VERUS, 'exact pinned verifier identity')
        summary = {'encountered-error': negative, 'encountered-vir-error': False,
                   'success': not negative, 'verified': 12 if negative else 13,
                   'errors': 1 if negative else 0, 'is-verifying-entire-crate': True}
        need(digest(report['verification-results']) == digest(summary), 'whole-root obligation counts')
        functions = report['func-details']
        need(type(functions) is dict and 2 <= len(functions) <= 512, 'bounded complete function roster')
        for name, notes in functions.items():
            need(type(name) is str and notes == {'obligation_proof_notes': [], 'failed_proof_notes': []},
                 'no omitted, selected, trusted, or failed proof notes')
        for name in ('RuntimeContextV1::has_unpublished_holds_v1', 'RuntimeContextV1::pending_replicas_v1',
                     'RuntimeContextV1::release_graph_after_access_v1', 'RuntimeReplicaStorageV1::usage',
                     'map_scan_equivalence', 'retirement_cases_are_inhabited'):
            need('graph_reservation_retirement_v1::' + name in functions, 'all concrete retirement checks present')
        rows = normalized_diagnostics(stderr, root, files)
        if negative:
            errors = [row for row in rows if row['level'] == 'error' and row['message'] in LOGICAL]
            need(errors and all(any(span.get('is_primary') is True for span in row['spans'])
                                for row in errors), 'actual located logical failure')
        else:
            need(not rows and not stderr, 'positive has no diagnostics')
        need(set(expected) == {'stdout', 'diagnostics'} and digest(report) == expected['stdout']
             and digest(rows) == expected['diagnostics'], 'exact calibrated whole-root diagnostic shape')
        return True
    except (ValueError, TypeError, KeyError, AttributeError, OverflowError):
        return False
