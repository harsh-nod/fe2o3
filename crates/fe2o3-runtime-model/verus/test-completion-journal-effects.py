#!/usr/bin/env python3
"""CPU calibration of source, mutation, scope, and evidence refusal gates."""
import copy
from pathlib import Path
import runpy
import sys
import tempfile

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

HERE = Path(__file__).resolve().parent
C = runpy.run_path(str(HERE / 'check-completion-journal-effects.py'))


def reject(function, *args):
    try:
        function(*args)
    except ValueError:
        return
    raise AssertionError('accepted corrupt completion evidence')


def main():
    repo = HERE.parents[2]
    loaded = C['load'](repo)
    control, owner, old, lifecycle, deps, *_, paths = loaded
    candidate = {p: old.ordinary(repo, p) for p in paths}
    implementation = {p: old.git(repo, 'show', C['IMPLEMENTATION'] + ':' + str(p))
                      for p in paths - {C['CHECK'], C['TEST']}}
    owner_data, control_data = C['historical'](repo, loaded, None)
    gate = C['source_gate']
    args = (implementation, owner_data, control_data, loaded)

    # 1. Closed inventory and independent historical source authentication.
    gate(candidate, *args)
    reject(gate, candidate | {Path('foreign.rs'): b''}, *args)
    for path in (C['ROOT'], C['EFFECTS'], control.PREVIOUS, deps['legacy'].MANIFEST):
        reject(gate, {p: data for p, data in candidate.items() if p != path}, *args)
        reject(gate, candidate | {path: candidate[path] + b'\n'}, *args)

    # 2. Only the reviewed opacity insertion is permitted in inherited proofs.
    path = C['CONSTRUCTOR']
    text = candidate[path].decode()
    anchor = '    hide(logical::producer_invariant_v1);\n'
    assert text.count(anchor) == 1
    for replacement in ('', '    assume(false);\n', anchor + '    hide(producer_represents);\n'):
        reject(gate, candidate | {path: text.replace(anchor, replacement).encode()}, *args)
    for path in (C['EFFECTS'], C['WITNESSES'], C['PREFIX'], C['SETTLEMENT']):
        reject(gate, candidate | {path: candidate[path] + b'// unreviewed\n'}, *args)

    # 3. Each nested control mutant has the exact historical flattened defect.
    controls = C['control_mutations'](candidate, control_data[C['BODY']], control)
    assert len(controls) == 22
    assert {name.removeprefix('control-') for name in controls} == set(control.mutations(control_data[C['BODY']].decode()))
    swap = controls['control-swap-1'][0]
    assert set(swap) == {C['BODY'], C['PREFIX']}
    assert b'release_operation_dependencies_v1' not in swap[C['BODY']]
    prefix = swap[C['PREFIX']]
    assert prefix.index(b'release_submission_inputs_v1') < prefix.index(b'release_operation_dependencies_v1') < prefix.index(b'settle_submission_writer_v1')
    for edits, module, function in controls.values():
        assert set(edits) <= {C['BODY'], C['PREFIX']}
        assert module == '' and function == 'SettlementProbeV1::settle_v1'
        reject(gate, candidate | edits, *args)

    # 4. Effect mutations cannot change the independent logical oracle.
    effects = C['effect_mutations'](candidate)
    assert len(effects) == 8
    assert len({tuple(sorted((str(p), data) for p, data in edits.items())) for edits, _, _ in effects.values()}) == 8
    for edits, module, function in effects.values():
        assert len(edits) == 1 and set(edits) <= {C['PREFIX'], C['SETTLEMENT']}
        assert module == 'production::completion' and function
        assert (candidate | edits)[C['EFFECTS']] == candidate[C['EFFECTS']]
        reject(gate, candidate | edits, *args)

    # 5. Source policy also scans every compile-clean mutation projection.
    changes = controls | effects
    with tempfile.TemporaryDirectory(prefix='completion-effect-calibration-') as temporary:
        stage = Path(temporary)
        for change in [None, *changes.values()]:
            owner.materialize(stage, candidate | (change[0] if change else {}))
            C['scan'](stage, deps)
        for path, addition in ((C['EFFECTS'], '\nproof fn bad() { assume(false); }\n'),
                               (C['SETTLEMENT'], '\ninclude!("foreign.rs");\n'),
                               (C['WITNESSES'], '\n#[verifier::external_body] proof fn bad() {}\n')):
            owner.materialize(stage, candidate | {path: candidate[path] + addition.encode()})
            try:
                C['scan'](stage, deps)
            except deps['policy'].ScanError:
                pass
            else:
                raise AssertionError('accepted forbidden proof source')

    # 6. Command scope, roots and budgets are authoritative, not a JSON flag.
    command = C['proof_command']
    verus, stage = Path('/pinned/verus'), Path('/owned/sources-test')
    for name in ('effects-positive-before', 'effects-positive-after', 'control-positive-before',
                 'control-positive-after', 'owner-regression', *changes):
        result = command(loaded, verus, stage, name, changes)
        expected_root = control.ROOT if name.startswith('control-') else owner.ROOT if name == 'owner-regression' else C['ROOT']
        assert result[-1] == str(stage / expected_root)
        expected_budget = 120 if name.startswith('control-') else 600 if name in changes else 1200
        assert result[4] == str(expected_budget)
        assert '--no-cheating' in result and '--rlimit' not in result
        assert ('--verify-function' in result) == (name in changes)
        if name in changes:
            assert result[result.index('--verify-function') + 1] == changes[name][2]
            if name.startswith('effect-'):
                assert result[result.index('--verify-only-module') + 1] == 'production::completion'
        else:
            assert '--verify-only-module' not in result

    # 7. Syntax, resource, timeout and mixed logical/resource failures refuse.
    scalar = deps['scalar']
    observed = {'result': {'encountered-error': True, 'encountered-vir-error': False,
        'verified': 0, 'errors': 1, 'is-verifying-entire-crate': False},
        'diagnostics': [{'level': 'error', 'message': 'postcondition not satisfied'}]}
    lifecycle.check_negative(scalar, 'effect-skip-stable', 1, observed)
    for status in (0, True, 124, -9):
        reject(lifecycle.check_negative, scalar, 'effect-skip-stable', status, observed)
    for message in ('unexpected token', 'function body check: Resource limit (rlimit) exceeded'):
        bad = copy.deepcopy(observed)
        bad['diagnostics'].append({'level': 'error', 'message': message})
        reject(lifecycle.check_negative, scalar, 'effect-skip-stable', 1, bad)
    for key, value in (('encountered-vir-error', True), ('errors', 0), ('verified', False),
                       ('is-verifying-entire-crate', True)):
        bad = copy.deepcopy(observed)
        bad['result'][key] = value
        reject(lifecycle.check_negative, scalar, 'effect-skip-stable', 1, bad)

    # 8. Receipt time types, order and hard execution bounds remain checked.
    row = {'started_ns': 10, 'finished_ns': 20}
    owner.receipt_budget(row, 120)
    for bad in (row | {'started_ns': True}, row | {'finished_ns': 1}, row | {'finished_ns': 136 * 10**9}):
        reject(owner.receipt_budget, bad, 120)
    print('PASS: completion journal effects calibration (8 groups)')


if __name__ == '__main__':
    main()
