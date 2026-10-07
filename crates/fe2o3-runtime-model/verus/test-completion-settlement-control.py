#!/usr/bin/env python3
"""CPU-only calibration of the bounded completion-control campaign."""
import copy
from pathlib import Path
import runpy
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

HERE = Path(__file__).resolve().parent
C = runpy.run_path(str(HERE / 'check-completion-settlement-control.py'))


def reject(function, *args):
    try:
        function(*args)
    except ValueError:
        return
    raise AssertionError('accepted corrupt control')


def main():
    repo = HERE.parents[2]
    owner, old, lifecycle, deps = C['load'](repo)
    legacy = deps['legacy']
    tools = {p: old.ordinary(repo, p) for p in (legacy.CLOSURE, legacy.MANIFEST)}
    C['tool_gate'](tools, legacy)
    for path in tools:
        reject(C['tool_gate'], tools | {path: tools[path] + b'\n'}, legacy)
    candidate = {p: old.ordinary(repo, p) for p in (C['BODY'], C['ROOT'], C['CONTEXT'])}
    baseline = old.git(repo, 'show', C['BASELINE'] + ':' + str(C['CONTEXT']))
    gate = C['source_gate']
    gate(candidate, baseline)
    for path in (C['BODY'], C['ROOT']):
        bad = candidate | {path: candidate[path] + b'\n'}
        reject(gate, bad, baseline)
    text = candidate[C['CONTEXT']].decode()
    for before, after in (
        ('include!("context/completion_settlement_body.rs");', ''),
        ('include!("context/completion_settlement_body.rs");', 'include!("foreign.rs");'),
        ('completion_settlement_execution_body!(', 'unreviewed_settlement_body!('),
        ('        self.check_operation_custody_v1(submission)?;\n', ''),
        ('        completion_settlement_execution_body!(',
         '        self.publish_submission_status_v1(submission, status)?;\n        completion_settlement_execution_body!('),
    ):
        assert before in text
        reject(gate, candidate | {C['CONTEXT']: text.replace(before, after).encode()}, baseline)

    body = candidate[C['BODY']].decode()
    mutations = C['mutations'](body)
    assert len(mutations) == 22
    for name, mutated in mutations.items():
        assert mutated != body and 'macro_rules! completion_settlement_execution_body' in mutated, name
        # Even a newly computed artifact hash cannot override reviewed body pins.
        assert C['sha'](mutated.encode()) != C['PINS'][C['BODY']]
        reject(gate, candidate | {C['BODY']: mutated.encode()}, baseline)

    scalar = deps['scalar']
    observed = {'result': {'encountered-error': True, 'encountered-vir-error': False,
        'verified': 0, 'errors': 1, 'is-verifying-entire-crate': False},
        'diagnostics': [{'level': 'error', 'message': 'precondition not satisfied'}]}
    lifecycle.check_negative(scalar, 'swap-0', 1, observed)
    for status in (0, True, 124, -9):
        reject(lifecycle.check_negative, scalar, 'swap-0', status, observed)
    for message in ('unexpected token', 'function body check: Resource limit (rlimit) exceeded'):
        bad = copy.deepcopy(observed)
        bad['diagnostics'].append({'level': 'error', 'message': message})
        reject(lifecycle.check_negative, scalar, 'swap-0', 1, bad)
    for key, value in (('encountered-vir-error', True), ('errors', 0), ('verified', False),
                       ('is-verifying-entire-crate', True)):
        bad = copy.deepcopy(observed)
        bad['result'][key] = value
        reject(lifecycle.check_negative, scalar, 'swap-0', 1, bad)

    row = {'started_ns': 10, 'finished_ns': 20}
    owner.receipt_budget(row, 120)
    for bad in (row | {'started_ns': True}, row | {'finished_ns': 1},
                row | {'finished_ns': 136 * 10**9}):
        reject(owner.receipt_budget, bad, 120)
    print('PASS: completion settlement control calibration (4 groups)')


if __name__ == '__main__':
    main()
