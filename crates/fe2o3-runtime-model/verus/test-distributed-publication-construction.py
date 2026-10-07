#!/usr/bin/env python3
"""Source/negative construction calibration; no solver or native execution."""
import importlib.util
from pathlib import Path
import sys
import types

if not (sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize):
    raise ValueError('use isolated python3 -I -B without optimization')

PATH = Path(__file__).with_name('check-distributed-publication-construction.py')
spec = importlib.util.spec_from_file_location('distributed_construction_checks', PATH)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

def refused(call):
    try:
        call()
    except (ValueError, AssertionError):
        return
    raise AssertionError('expected strict refusal')

def main():
    c.invocation_guard()
    valid = dict(isolated=1, dont_write_bytecode=1, optimize=0)
    c.invocation_guard(types.SimpleNamespace(**valid))
    for key, value in [('isolated', 0), ('dont_write_bytecode', 0), ('optimize', 1), ('optimize', 2)]:
        refused(lambda field=key, setting=value: c.invocation_guard(types.SimpleNamespace(**dict(valid, **{field: setting}))))
    refused(lambda: c.invocation_guard(types.SimpleNamespace()))
    source = c.snapshot()
    c.audit(source)
    for path in (c.OWNER, c.BODY, c.PROOF, c.OLD_PROOF, c.DECLARATIONS, c.OLD_BODY, c.IDENTITY):
        changed = dict(source)
        changed[path] += '\n'
        refused(lambda value=changed: c.audit(value))
    missing = dict(source)
    del missing[c.BODY]
    refused(lambda: c.audit(missing))
    body = source[c.BODY]
    cases = c.mutations(body)
    assert len(cases) == 30 and len(set(cases.values())) == 30
    assert {focus for _, focus in cases.values()} == set(c.SELECTORS.values())
    assert sum(focus == c.SELECTORS['binding'] for _, focus in cases.values()) == 18
    assert sum(focus == c.SELECTORS['receipt'] for _, focus in cases.values()) == 4
    assert sum(focus == c.SELECTORS['trailer'] for _, focus in cases.values()) == 8
    for value, _ in cases.values():
        assert value != body and value.count('macro_rules! ') == 3
    refused(lambda: c.mutations(body.replace('if $sequence == 0 {', 'if $sequence != 0 {')))
    assert 'let _ = &$outcome' in cases['receipt-outcome-substituted'][0]
    assert 'let _ = (a, b, c)' in cases['reserved-trailer-accepted'][0]
    assert c.EXPECTED_VERIFIED == 37
    expected_notes = frozenset(('verifying root module (selected functions)',))
    assert set(c.SELECTION_NOTES) == set(c.SELECTORS.values())
    for focus in c.SELECTORS.values():
        assert c.selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), focus).SELECTION_NOTES == expected_notes
    for focus in ('*wrong_selector', None):
        refused(lambda value=focus: c.selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), value))
    observed_notes = c.SELECTION_NOTES
    try:
        focus = c.SELECTORS['binding']
        for notes in (None, {}, {focus: expected_notes}, dict(observed_notes, **{'*wrong_selector': expected_notes})):
            c.SELECTION_NOTES = notes
            refused(lambda: c.selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), focus))
        for notes in (set(expected_notes), frozenset(), frozenset(('unobserved note',)), expected_notes | {'unobserved note'}):
            c.SELECTION_NOTES = dict(observed_notes, **{focus: notes})
            refused(lambda: c.selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), focus))
    finally:
        c.SELECTION_NOTES = observed_notes
    assert c.campaign().EXPECTED['verified'] == 37
    native = source[c.OWNER]
    assert native.index('let sequence = r.u64()?;') < native.index('let outcome = decode_outcome_trailer(r.fixed()?)?;')
    assert native.count('distributed_receipt_classifier_body_v1!(record, receipt)') == 1
    assert source[c.DECLARATIONS].count('#[derive(Debug, Eq, PartialEq)]') == 1
    assert c.sha(source[c.OLD_PROOF]) == c.UNCHANGED[c.OLD_PROOF]
    print('PASS: distributed construction calibration (5 groups; 30 mutants constructed, not executed)')

main()
