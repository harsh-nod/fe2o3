#!/usr/bin/env python3
"""Source and classifier calibration only; never starts Cargo or Verus."""
import ast
import hashlib
import json
from pathlib import Path
import sys
import types

def need(value, message):
    if not value:
        raise AssertionError(message)

def refused(action):
    try:
        action()
    except ValueError:
        return
    raise AssertionError('hostile source, selector or unmeasured campaign accepted')

need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, 'python3 -I -B required')
path = Path(__file__).resolve().with_name('check-distributed-publication-contract.py')
c = types.ModuleType('distributed_publication_controls')
c.__file__ = str(path)
sys.modules[c.__name__] = c
exec(compile(path.read_bytes(), str(path), 'exec'), c.__dict__)
sources = c.snapshot()
c.audit(sources)

for path, before, after in (
    (c.IDENTITY, 'IDENTITY_DIGEST_BYTES_V1: usize = 32;', 'IDENTITY_DIGEST_BYTES_V1: usize = 8;'),
    (c.OWNER, 'pub struct $name(IdentityDigestV1);', 'pub struct $name(u64);'),
    (c.OWNER, 'distributed_receipt_classifier_body_v1!(record, receipt)', 'Ok(ModelDistributedReceiptDispositionV1::Recorded)'),
    (c.DECLARATIONS, '    pub runtime_model: RuntimeModelIdV1,\n', ''),
    (c.DECLARATIONS, '#[derive(Debug, Eq, PartialEq)]\npub struct ModelDistributedPublicationRecordV1',
     '#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub struct ModelDistributedPublicationRecordV1'),
    (c.PROOF, 'left.runtime_model.0.0 == right.runtime_model.0.0', 'true'),
    (c.PROOF, 'pub struct IdentityDigestV1([u8; 32]);', 'pub struct IdentityDigestV1(u64);'),
    (c.PROOF, 'final(self).last == old(self).last,', 'true,'),
    (c.BODY, 'if $record.binding != $receipt.binding', 'if false'),
):
    need(sources[path].count(before) == 1, 'nonvacuous source mutation')
    refused(lambda: c.audit({**sources, path: sources[path].replace(before, after)}))
for missing in (c.OWNER, c.IDENTITY, c.DECLARATIONS, c.BODY, c.PROOF):
    reduced = dict(sources)
    del reduced[missing]
    refused(lambda: c.audit(reduced))
refused(lambda: c.audit({**sources, c.SRC / 'unbound_distributed.rs': '// unbound\n'}))
refused(lambda: c.audit({**sources, c.V / 'unbound_distributed.rs': '// unbound\n'}))

cases = c.mutations(sources[c.BODY])
need(len(cases) == c.MUTANT_COUNT == 16, 'sixteen proposed actual-body cases')
need([sum(focus == c.SELECTORS[kind] for _, focus in cases.values()) for kind in ('classify', 'record', 'observe')]
     == [10, 3, 3], 'exact actual classifier/record/observation split')
need(len({body for body, _ in cases.values()}) == 16, 'distinct executable body changes')
need('if { let _ = next; false } {' in cases['successor-gap-accepted'][0], 'unused-binding refusal is not the arithmetic mutation')
for name, (body, focus) in cases.items():
    need(body != sources[c.BODY] and focus in c.SELECTORS.values(), 'real mutation: ' + name)
    need('assume(' not in body and 'admit(' not in body, 'no proof-only premises')
    refused(lambda: c.audit({**sources, c.BODY: body}))
refused(lambda: c.change(sources[c.BODY], 'not_a_macro', 'missing', 'other'))
refused(lambda: c.change(sources[c.BODY], 'distributed_observation_body_v1', 'missing', 'other'))

measured_notes = {focus: frozenset({'verifying root module (selected functions)'}) for focus in c.SELECTORS.values()}
need(c.EXPECTED_VERIFIED == 23 and c.SELECTION_NOTES == measured_notes, 'measured full count and exact reviewed per-selector notes')
leaf = types.SimpleNamespace(LOGICAL_ERRORS={'postcondition not satisfied', 'precondition not satisfied'})
for focus in c.SELECTORS.values():
    need(c.selection_notes(leaf, focus).SELECTION_NOTES == measured_notes[focus], 'exact measured selector singleton')
for focus in ('*', '*decode_untrusted_description', '*record_typo'):
    refused(lambda: c.selection_notes(leaf, focus))
try:
    for malformed in (None, {}, {c.SELECTORS['classify']: frozenset()},
                      {focus: frozenset() for focus in measured_notes}):
        c.SELECTION_NOTES = malformed
        refused(c.campaign)
finally:
    c.SELECTION_NOTES = measured_notes
controller = c.campaign()
need(controller.FILES == c.FILES and controller.PROOF == c.PROOF and controller.BODY == c.BODY
     and controller.EXPECTED['verified'] == 23, 'exact source closure and measured full campaign count')
need(hashlib.sha256((c.ROOT / c.BASE).read_bytes()).hexdigest() == c.BASE_SHA, 'unchanged managed campaign source')
ast.parse((c.ROOT / c.BASE).read_bytes())
classifier = controller.inherited()
verifier = {'fixture': 'source-calibration-only'}
paths = {str(c.ROOT / p) for p in c.FILES}
notes = c.selection_notes(leaf, c.SELECTORS['classify'])
negative = {'verus': verifier, 'verification-results': {'encountered-error': True,
    'encountered-vir-error': False, 'verified': 0, 'errors': 1, 'is-verifying-entire-crate': False}}
diagnostic = {'$message_type': 'diagnostic', 'message': 'postcondition not satisfied', 'level': 'error',
              'code': None, 'children': [], 'spans': [{'file_name': str(c.ROOT / c.PROOF), 'is_primary': True}]}

def accepted(value=negative, error=diagnostic, status=1):
    return classifier.logical_negative(notes, status, json.dumps(value), json.dumps(error), verifier, paths)

need(accepted(), 'nonvacuous synthetic logical-negative fixture')
for status in (0, 101, 124, -9):
    need(not accepted(status=status), 'nonlogical exit refused')
for patch in ({'message': 'mismatched types', 'code': {'code': 'E0308'}}, {'level': 'warning'},
              {'message': 'timed out'}, {'spans': []},
              {'spans': [{'file_name': '/tmp/unrelated.rs', 'is_primary': True}]}):
    need(not accepted(error={**diagnostic, **patch}), 'compiler/warning/timeout/source mismatch refused')
for key, value in (('encountered-vir-error', True), ('errors', 0), ('verified', True),
                   ('is-verifying-entire-crate', True), ('success', False)):
    need(not accepted(value={**negative, 'verification-results': {**negative['verification-results'], key: value}}), 'malformed result refused')
unknown = {**diagnostic, 'level': 'note', 'message': 'unmeasured selected diagnostic'}
measured = {**unknown, 'message': 'verifying root module (selected functions)', 'spans': []}
for focus in c.SELECTORS.values():
    need(classifier.logical_negative(c.selection_notes(leaf, focus), 1, json.dumps(negative),
         json.dumps(diagnostic) + '\n' + json.dumps(measured), verifier, paths), 'nonvacuous measured selector-note fixture')
need(not classifier.logical_negative(notes, 1, json.dumps(negative),
     json.dumps(diagnostic) + '\n' + json.dumps(unknown), verifier, paths), 'unknown note refused')
print('PASS: distributed publication contract source/classifier calibration (4 groups; 16 mutants constructed, not executed)')
