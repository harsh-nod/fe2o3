#!/usr/bin/env python3
"""Source and mutation construction controls, not proof execution."""
import importlib.util
import copy
import io
import json
from pathlib import Path
import sys
import unittest

BASE = Path(__file__).resolve().parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, BASE / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SOURCE = load('retirement_source_control', 'check-graph-reservation-retirement-v1.py')
MUTATIONS = load('retirement_mutation_control', 'graph-reservation-retirement-mutations-v1.py')
DIAGNOSTICS = load('retirement_diagnostic_control', 'graph-reservation-retirement-diagnostics-v1.py')
CAMPAIGN = load('retirement_campaign_control', 'qualify-graph-reservation-retirement-v1.py')


class SourceControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.values = SOURCE.snapshot()

    def refused(self, path, before, after):
        self.assertEqual(self.values[path].count(before), 1)
        changed = dict(self.values)
        changed[path] = changed[path].replace(before, after)
        with self.assertRaises(ValueError):
            SOURCE.source_shapes(changed)

    def test_exact_source_shapes(self):
        SOURCE.source_shapes(self.values)

    def test_native_field_roster_and_model_frames_are_closed(self):
        for path, before, after in (
            (SOURCE.CONTEXT, '    replicas: Option<RuntimeReplicaStorageV1>,\n', ''),
            (SOURCE.CONTEXT, '    completion_callback_count: usize,', '    completion_callback_count: u64,'),
            (SOURCE.DEFINITIONS, '    replicas: Option<RuntimeReplicaStorageV1<O>>,\n', ''),
            (SOURCE.DEFINITIONS, '        &&& self.replicas == before.replicas\n', ''),
            (SOURCE.DEFINITIONS, 'credits: Option<C>', 'credits: ()'),
            (SOURCE.STORAGE, '    pub(super) incarnation: u64,', '    pub(super) incarnation: u32,'),
            (SOURCE.GRAPH, '#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub(crate) struct ContextGraphReservationV1',
             '#[derive(Clone, Copy, Debug)]\npub(crate) struct ContextGraphReservationV1'),
        ):
            with self.subTest(before=before):
                self.refused(path, before, after)

    def test_original_access_prefix_and_concrete_scans_cannot_be_bypassed(self):
        for path, before, after in (
            (SOURCE.GRAPH, 'self.require_graph_access(Some(token))?;\n        use retirement_bodies',
             'use retirement_bodies'),
            (SOURCE.UNPUBLISHED, 'graph_unpublished_holds_body_v1!(', 'other_unpublished_holds!('),
            (SOURCE.REPLICAS, 'graph_pending_replicas_body_v1!(', 'other_pending_replicas!('),
            (SOURCE.STORAGE, '(index, pending, settled, count)', '(index, settled, pending, count)'),
            (SOURCE.BODY, '        $expression\n', '        false\n'),
            (SOURCE.DEFINITIONS, 'fn obeys_eq_spec() -> bool { true }', 'fn obeys_eq_spec() -> bool { false }'),
        ):
            with self.subTest(before=before):
                self.refused(path, before, after)

    def test_inactive_or_substituted_module_edges_refuse(self):
        for path, declaration in ((SOURCE.GRAPH, 'pub(super) mod retirement_bodies;'),
                                  (SOURCE.REPLICAS, 'mod storage;'),
                                  (SOURCE.CONTEXT, 'mod replicas;')):
            for prefix in ('#[cfg(any())]\n', '#[path = "shadow.rs"]\n'):
                with self.subTest(declaration=declaration, prefix=prefix):
                    self.refused(path, declaration, prefix + declaration)

    def test_no_added_proof_trust_or_library_shadow(self):
        for addition in ('\nfn bypass() { assume(false); }\n', '\nmod std {}\n',
                         '\n#[verifier::external_body] fn bypass() {}\n'):
            changed = dict(self.values)
            changed[SOURCE.DEFINITIONS] += addition
            with self.assertRaises(ValueError):
                SOURCE.source_shapes(changed)

    def test_23_distinct_body_only_mutations(self):
        body = self.values[SOURCE.BODY]
        cases = MUTATIONS.mutations(body)
        self.assertEqual(len(cases), 23)
        self.assertEqual(len({row['text'] for row in cases.values()}), 23)
        policy = DIAGNOSTICS.parse(SOURCE.ordinary(BASE / 'pins/GRAPH_RESERVATION_RETIREMENT_DIAGNOSTICS_V1.json').decode())
        self.assertEqual(set(policy), {'positive', *cases})
        for row in policy.values():
            self.assertEqual(set(row), {'stdout', 'diagnostics'})
            self.assertTrue(all(len(value) == 64 and int(value, 16) >= 0 for value in row.values()))
        for name, row in cases.items():
            with self.subTest(name=name):
                self.assertNotEqual(row['text'], body)
                self.assertIn(row['after'], row['text'])
                self.assertEqual(set(row), {'method', 'before', 'after', 'text'})

    def test_diagnostics_are_closed_and_original_limits_retained(self):
        command = CAMPAIGN.proof_command(Path('/pinned/verus'), SOURCE.ROOT, SOURCE)
        self.assertEqual(command, ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5',
            '120', '/pinned/verus', '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating',
            '--output-json', '--error-format=json', '--num-threads', '4', str(SOURCE.ROOT / SOURCE.PROOF)])
        for negative in (False, True):
            functions = ('RuntimeContextV1::has_unpublished_holds_v1', 'RuntimeContextV1::pending_replicas_v1',
                         'RuntimeContextV1::release_graph_after_access_v1', 'RuntimeReplicaStorageV1::usage',
                         'map_scan_equivalence', 'retirement_cases_are_inhabited')
            report = {'verus': DIAGNOSTICS.VERUS, 'verification-results': {
                'encountered-error': negative, 'encountered-vir-error': False, 'success': not negative,
                'verified': 12 if negative else 13, 'errors': 1 if negative else 0,
                'is-verifying-entire-crate': True}, 'func-details': {
                    'graph_reservation_retirement_v1::' + name:
                    {'obligation_proof_notes': [], 'failed_proof_notes': []} for name in functions}}
            rows = [] if not negative else [{'$message_type': 'diagnostic', 'code': None, 'level': 'error',
                'message': 'assertion failed', 'children': [], 'rendered': 'error: assertion failed',
                'spans': [{'file_name': str(SOURCE.ROOT / SOURCE.PROOF), 'is_primary': True}]}]
            stdout = json.dumps(report)
            stderr = '\n'.join(json.dumps(row) for row in rows)
            expected = {'stdout': DIAGNOSTICS.digest(report), 'diagnostics': DIAGNOSTICS.digest(
                DIAGNOSTICS.normalized_diagnostics(stderr, SOURCE.ROOT, SOURCE.PROOF_FILES))}
            status = 1 if negative else 0
            check = lambda s, o, e: DIAGNOSTICS.classify(s, o, e, SOURCE.ROOT, SOURCE.PROOF_FILES, expected, negative)
            self.assertTrue(check(status, stdout, stderr))
            for wrong in (-15, -9, 124, 125, 127, 2, True, 0 if negative else 1):
                self.assertFalse(check(wrong, stdout, stderr))
            for key, value in (('verified', 999), ('verified', True), ('errors', 0 if negative else 1),
                               ('encountered-vir-error', True), ('is-verifying-entire-crate', False)):
                revised = copy.deepcopy(report)
                revised['verification-results'][key] = value
                self.assertFalse(check(status, json.dumps(revised), stderr))
            revised = copy.deepcopy(report)
            revised['verus']['version'] = 'unreviewed'
            self.assertFalse(check(status, json.dumps(revised), stderr))
            revised = copy.deepcopy(report)
            del revised['func-details']['graph_reservation_retirement_v1::RuntimeReplicaStorageV1::usage']
            self.assertFalse(check(status, json.dumps(revised), stderr))
            self.assertFalse(check(status, stdout.replace('"func-details":', '"func-details":{},"func-details":'), stderr))
            self.assertFalse(check(status, stdout, stderr + '\nnot JSON'))
            if negative:
                for message in ('out of resource', 'timed out', 'mismatched types', 'assertion failed elsewhere'):
                    changed = copy.deepcopy(rows)
                    changed[0]['message'] = message
                    self.assertFalse(check(status, stdout, json.dumps(changed[0])))
                changed = copy.deepcopy(rows)
                changed[0]['spans'][0]['file_name'] = '/tmp/foreign.rs'
                self.assertFalse(check(status, stdout, json.dumps(changed[0])))
                changed[0]['spans'][0]['file_name'] = str(SOURCE.ROOT / SOURCE.PROOF)
                changed[0]['spans'][0]['is_primary'] = False
                self.assertFalse(check(status, stdout, json.dumps(changed[0])))
                self.assertFalse(check(status, stdout, stderr + '\n' + stderr))
            else:
                self.assertFalse(check(status, stdout, '\n'))


if __name__ == '__main__':
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
        raise SystemExit('python3 -I -B required')
    log = io.StringIO()
    result = unittest.TextTestRunner(stream=log).run(unittest.defaultTestLoader.loadTestsFromTestCase(SourceControls))
    if not result.wasSuccessful():
        sys.stderr.write(log.getvalue())
        raise SystemExit(1)
    print('PASS: executable graph retirement source and diagnostic controls (7 groups)')
