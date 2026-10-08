#!/usr/bin/env python3
"""CPU-only source/diagnostic controls; constructs mutants but runs no prover."""
import copy
import importlib.util
import json
from pathlib import Path
import sys

BASE = Path(__file__).resolve().parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, BASE / filename)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


S = load('ledger_test_sources', 'check-graph-version-ledger-v1.py')
D = load('ledger_test_diagnostics', 'graph-version-ledger-diagnostics-v1.py')
M = load('ledger_test_mutations', 'graph-version-ledger-mutations-v1.py')
C = load('ledger_test_campaign', 'qualify-graph-version-ledger-v1.py')


def refuses(call):
    try:
        call()
    except (ValueError, KeyError):
        return
    raise AssertionError('hostile source accepted')


def changed(values, path, before, after):
    assert values[path].count(before) == 1, before
    result = dict(values)
    result[path] = result[path].replace(before, after)
    return result


def source_controls(values):
    S.audit(values)
    C.support_check(S)
    parser = S.lexer()
    for name, kind in S.fields(values[S.NATIVE], 'VersionLedger', parser).items():
        declaration = next(line for line in values[S.NATIVE].splitlines()
                           if ''.join(line.split()) == name + ':' + kind + ',')
        refuses(lambda: S.source_shapes(changed(values, S.NATIVE, declaration, '')))
        refuses(lambda: S.source_shapes(changed(values, S.NATIVE, declaration, '    ' + name + ': bool,')))
    for path, before, after in (
        (S.NATIVE, 'include!("versions/transition_bodies.rs");',
         '#[cfg(any())]\ninclude!("versions/transition_bodies.rs");'),
        (S.NATIVE, 'graph_version_begin_body_v1!(', 'unrelated_begin!('),
        (S.PROOF, 'graph_version_begin_body_v1!(verus_exec_expr, self, node,',
         'graph_version_begin_body_v1!(untrusted_exec, self, node,'),
        (S.MODEL_ROOT, 'mod r65_graph_versions;', '#[cfg(any())]\nmod r65_graph_versions;'),
        (S.GRAPH_ROOT, 'mod versions;', '#[path="unrelated.rs"]\nmod versions;'),
        (S.ASYNC_ROOT, 'mod graph;', '#[cfg(any())]\nmod graph;'),
        (S.RUNTIME_ROOT, 'mod async_engine;', '#[cfg(any())]\nmod async_engine;'),
        (S.DEFINITIONS, 'current: Vec<Option<usize>>,', 'current: Vec<Option<u32>>,'),
        (S.DEFINITIONS, 'expected: usize,', 'expected: u32,'),
        (S.DEFINITIONS, 'use vstd::prelude::*;', 'use vstd::prelude::*;\ntype Vec<T> = T;'),
        (S.DEFINITIONS, 'use vstd::prelude::*;', 'use vstd::prelude::*;\nuse malicious::Array as Vec;'),
        (S.DEFINITIONS, 'use vstd::prelude::*;', 'use vstd::prelude::*;\nuse malicious::verus_exec_expr;'),
        (S.PROOF, 'verus! {', 'verus! {\n#[verifier::external_body]'),
        (S.ADMITTED, 'fn succeed_host_staging(', 'fn bypass_host_staging('),
        (S.STAGING, 'write_host_visible_with_graph_access_v1', 'unrelated_write'),
    ):
        # Schema checks run even with an adversarially refreshed outer byte pin.
        if values[path].count(before) == 1:
            refuses(lambda: S.source_shapes(changed(values, path, before, after)))
        else:
            raise AssertionError('stale hostile source case: ' + before)
    for text in ('struct Vec<T> { value: T }', 'use malicious::Slot as Option;', 'mod vstd {}'):
        revised = dict(values)
        revised[S.DEFINITIONS] += '\n' + text + '\n'
        refuses(lambda: S.source_shapes(revised))
    for method in ('begin', 'commit', 'fail'):
        path = S.NATIVE if method == 'fail' else S.CPU_TEST
        name = method if method == 'fail' else 'predecessor_' + method
        revised = dict(values)
        opening = revised[path].index('{', revised[path].index('fn ' + name + '('))
        revised[path] = revised[path][:opening + 1] + ' let _ = 1; ' + revised[path][opening + 1:]
        refuses(lambda: S.source_shapes(revised))


def mutation_controls(values):
    cases = M.mutations(values[S.BODY])
    assert len(cases) == len({row['text'] for row in cases.values()}) == 18
    assert {row['method'] for row in cases.values()} == {'begin', 'commit'}
    for row in cases.values():
        assert row['text'] != values[S.BODY] and row['before'] != row['after']
        assert S.item_body(values[S.PROOF], 'fn', row['method'], S.lexer())
    policy = S.unique_json(S.ordinary(BASE / 'pins/GRAPH_VERSION_LEDGER_DIAGNOSTICS_V1.json'))
    assert set(policy) == {'positive', *cases}
    assert all(set(row) == {'stdout', 'diagnostics'} and
               all(len(value) == 64 and int(value, 16) >= 0 for value in row.values())
               for row in policy.values())


def fixture(negative):
    report = {'verus': D.VERUS, 'verification-results': {
        'encountered-error': negative, 'encountered-vir-error': False, 'success': not negative,
        'verified': 12 if negative else 13, 'errors': 1 if negative else 0, 'is-verifying-entire-crate': True},
        'func-details': {'graph_version_ledger_v1::VersionLedger::' + name:
                         {'obligation_proof_notes': [], 'failed_proof_notes': []} for name in ('begin', 'commit')}}
    rows = [] if not negative else [{'$message_type': 'diagnostic', 'code': None, 'level': 'error',
            'message': 'assertion failed', 'children': [], 'rendered': 'error: assertion failed',
            'spans': [{'file_name': str(S.ROOT / S.PROOF), 'is_primary': True}]}]
    stdout = json.dumps(report)
    stderr = '\n'.join(json.dumps(row) for row in rows)
    expected = {'stdout': D.digest(report), 'diagnostics': D.digest(D.normalized_diagnostics(stderr, S.ROOT, S.PROOF_FILES))}
    return report, rows, stdout, stderr, expected


def diagnostic_controls():
    for negative in (False, True):
        report, rows, stdout, stderr, expected = fixture(negative)
        status = 1 if negative else 0
        check = lambda s, o, e: D.classify(s, o, e, S.ROOT, S.PROOF_FILES, expected, negative)
        assert check(status, stdout, stderr)
        for wrong_status in (-15, -9, 124, 125, 127, 2, True, 0 if negative else 1):
            assert not check(wrong_status, stdout, stderr)
        for key, value in (('verified', 999), ('verified', True), ('errors', 0 if negative else 1),
                           ('encountered-vir-error', True), ('is-verifying-entire-crate', False)):
            revised = copy.deepcopy(report)
            revised['verification-results'][key] = value
            assert not check(status, json.dumps(revised), stderr)
        revised = copy.deepcopy(report)
        revised['verus']['version'] = 'unreviewed'
        assert not check(status, json.dumps(revised), stderr)
        revised = copy.deepcopy(report)
        del revised['func-details']['graph_version_ledger_v1::VersionLedger::commit']
        assert not check(status, json.dumps(revised), stderr)
        assert not check(status, stdout.replace('"func-details":', '"func-details":{},"func-details":'), stderr)
        assert not check(status, stdout, stderr + '\nnot JSON')
        if negative:
            for message in ('out of resource', 'timed out', 'mismatched types', 'assertion failed elsewhere'):
                revised_rows = copy.deepcopy(rows)
                revised_rows[0]['message'] = message
                assert not check(status, stdout, json.dumps(revised_rows[0]))
            revised_rows = copy.deepcopy(rows)
            revised_rows[0]['spans'][0]['file_name'] = '/tmp/foreign.rs'
            assert not check(status, stdout, json.dumps(revised_rows[0]))
            revised_rows[0]['spans'][0]['file_name'] = str(S.ROOT / S.PROOF)
            revised_rows[0]['spans'][0]['is_primary'] = False
            assert not check(status, stdout, json.dumps(revised_rows[0]))
            assert not check(status, stdout, stderr + '\n' + stderr)
        else:
            assert not check(status, stdout, '\n')


def main():
    assert sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize
    values = S.snapshot()
    source_controls(values)
    mutation_controls(values)
    diagnostic_controls()
    command = C.proof_command(Path('/pinned/verus'), S.ROOT, S)
    assert command == ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '120',
                       '/pinned/verus', '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating',
                       '--output-json', '--error-format=json', '--num-threads', '4', str(S.ROOT / S.PROOF)]
    refuses(lambda: S.unique_json('{"files":{},"files":{}}'))
    assert S.snapshot() == values
    print('PASS: executable graph ledger source and diagnostic controls (6 groups)')


if __name__ == '__main__':
    main()
