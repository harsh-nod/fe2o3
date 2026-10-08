#!/usr/bin/env python3
"""Hostile classifier/source controls only; these never execute a verifier."""
import copy
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


diagnostics = load('cached_diagnostics_tests', HERE / 'context-cached-poll-diagnostics-v1.py')
mutations = load('cached_mutations_tests', HERE / 'context-cached-poll-mutations-v1.py')
inherited = load('cached_source_tests', HERE / 'test-context-cached-poll-v1.py')


class DiagnosticControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.loaded = diagnostics.parse((HERE / 'pins/CONTEXT_CACHED_POLL_DIAGNOSTICS_V1.json').read_text())
        # Both mechanism fixtures are independent of the on-disk activation state.
        cls.draft = copy.deepcopy(cls.loaded)
        cls.draft['state'] = 'draft-review-required'
        cls.policy = copy.deepcopy(cls.loaded)
        cls.policy['state'] = 'reviewed-calibrated'
        cls.temp = tempfile.TemporaryDirectory(prefix='fe2o3-cached-classifier-')
        cls.root = Path(cls.temp.name)
        cls.sources = {}
        original = (ROOT / diagnostics.BODY).read_text()
        variants = mutations.mutations(original)
        for case in ('positive', *diagnostics.CASES):
            root = cls.root / case
            expected = cls.policy['positive'] if case == 'positive' else cls.policy['cases'][case]
            for name in expected['inputs']:
                path = HERE / 'pins/CONTEXT_CACHED_POLL_TOOLCHAIN.toml' if name == 'rust-toolchain.toml' else ROOT / name
                raw = path.read_bytes()
                if name == diagnostics.BODY and case != 'positive':
                    raw = variants[case]['text'].encode()
                destination = root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(raw)
                destination.chmod(expected['inputs'][name]['mode'])
            cls.sources[case] = root

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def material(self, case='held_stream_accepted'):
        expected = self.policy['positive'] if case == 'positive' else self.policy['cases'][case]
        root = self.sources[case]
        def restore(value, key=None):
            if isinstance(value, dict):
                return {k: restore(v, k) for k, v in value.items()}
            if isinstance(value, list):
                return [restore(v) for v in value]
            if isinstance(value, str):
                if key == 'file_name' and value.startswith('crates/'):
                    return str(root / value)
                return value.replace('<CAPTURED_SOURCE>', str(root))
            return value
        report = {'verus': diagnostics.VERUS, 'verification-results': expected['summary'],
                  'func-details': {name: {'obligation_proof_notes': [], 'failed_proof_notes': []}
                                   for name in expected['functions']}}
        rows = restore(copy.deepcopy(expected['warnings'] + expected['diagnostics']))
        return copy.deepcopy(report), rows

    def accepts(self, case, report, rows, status=None, policy=None):
        return diagnostics.classify(
            (0 if case == 'positive' else 1) if status is None else status,
            json.dumps(report), '\n'.join(json.dumps(row) for row in rows),
            self.sources[case], self.policy if policy is None else policy, case)

    def test_all_seventeen_complete_shapes(self):
        for case in ('positive', *diagnostics.CASES):
            with self.subTest(case=case):
                self.assertTrue(self.accepts(case, *self.material(case)))

    def test_draft_policy_cannot_launch(self):
        report, rows = self.material()
        self.assertFalse(self.accepts('held_stream_accepted', report, rows, policy=self.draft))

    def test_explicit_policy_states_preserve_loaded_input(self):
        self.assertEqual(self.draft['state'], 'draft-review-required')
        self.assertEqual(self.policy['state'], 'reviewed-calibrated')
        for fixture in (self.draft, self.policy):
            self.assertIsNot(fixture, self.loaded)
            self.assertEqual({k: v for k, v in fixture.items() if k != 'state'},
                             {k: v for k, v in self.loaded.items() if k != 'state'})
        with self.assertRaises(ValueError): diagnostics.policy_check(self.draft)
        diagnostics.policy_check(self.policy)
        self.assertTrue(self.accepts('held_stream_accepted', *self.material(), policy=self.policy))
        self.assertEqual(self.loaded, diagnostics.parse(
            (HERE / 'pins/CONTEXT_CACHED_POLL_DIAGNOSTICS_V1.json').read_text()))

    def test_exact_source_hash_and_mode(self):
        root = self.sources['held_stream_accepted']
        path = root / diagnostics.BODY
        raw = path.read_bytes()
        try:
            path.write_bytes(raw + b' ')
            self.assertFalse(self.accepts('held_stream_accepted', *self.material()))
            path.write_bytes(raw)
            path.chmod(0o600)
            self.assertFalse(self.accepts('held_stream_accepted', *self.material()))
        finally:
            path.write_bytes(raw)
            path.chmod(0o644)

    def test_named_omission_cannot_move_to_another_case(self):
        case = 'stream_hold_call_omitted'
        report, rows = self.material(case)
        del report['func-details'][diagnostics.OMISSION]
        self.assertFalse(self.accepts(case, report, rows))

    def test_private_selector_is_pinned_without_changing_repository_nightly(self):
        selector = HERE / 'pins/CONTEXT_CACHED_POLL_TOOLCHAIN.toml'
        import hashlib
        self.assertEqual(hashlib.sha256(selector.read_bytes()).hexdigest(),
                         '0b5ab62ff48b93d11576964057a24fe8122e76b50d797779e2b3b11f3e0e50f1')
        self.assertNotEqual((ROOT / 'rust-toolchain.toml').read_bytes(), selector.read_bytes())
        path = self.sources['held_stream_accepted'] / 'rust-toolchain.toml'
        before = path.read_bytes()
        try:
            path.write_bytes((ROOT / 'rust-toolchain.toml').read_bytes())
            self.assertFalse(self.accepts('held_stream_accepted', *self.material()))
        finally:
            path.write_bytes(before)

    def test_held_roster_requires_every_other_function(self):
        report, rows = self.material()
        for name in tuple(report['func-details']):
            changed = copy.deepcopy(report)
            del changed['func-details'][name]
            self.assertFalse(self.accepts('held_stream_accepted', changed, rows), name)

    def test_no_extra_function_or_trusted_notes(self):
        report, rows = self.material()
        report['func-details']['unreviewed'] = {'obligation_proof_notes': [], 'failed_proof_notes': []}
        self.assertFalse(self.accepts('held_stream_accepted', report, rows))
        report, rows = self.material()
        next(iter(report['func-details'].values()))['obligation_proof_notes'] = ['assumed']
        self.assertFalse(self.accepts('held_stream_accepted', report, rows))

    def test_wrong_flags_counts_or_exit(self):
        report, rows = self.material()
        for key in diagnostics.KEYS:
            changed = copy.deepcopy(report)
            old = changed['verification-results'][key]
            changed['verification-results'][key] = not old if type(old) is bool else old + 1
            self.assertFalse(self.accepts('held_stream_accepted', changed, rows))
        for code in (0, -9, 124, 137, 101, True):
            self.assertFalse(self.accepts('held_stream_accepted', report, rows, status=code))

    def test_warning_missing_duplicate_new_reordered(self):
        report, rows = self.material()
        variants = [rows[1:], [rows[0], *rows], [rows[1], rows[0], *rows[2:]],
                    [dict(rows[0], message='new warning'), *rows]]
        for changed in variants:
            self.assertFalse(self.accepts('held_stream_accepted', report, changed))

    def test_warning_text_level_site_path_and_unknown_field(self):
        report, rows = self.material()
        for mutation in ('message', 'level', 'site', 'path', 'extra', 'rendered'):
            changed = copy.deepcopy(rows)
            if mutation == 'site': changed[0]['spans'][0]['line_start'] += 1
            elif mutation == 'path': changed[0]['spans'][0]['file_name'] = '/rustc/not-source'
            elif mutation == 'rendered': changed[0]['rendered'] += 'changed'
            else: changed[0][mutation] = 'unreviewed'
            self.assertFalse(self.accepts('held_stream_accepted', report, changed))

    def test_positive_warning_sequence_stays_five(self):
        report, rows = self.material('positive')
        self.assertFalse(self.accepts('positive', report, rows[:2]))

    def test_logical_virtual_path_is_not_warning_authority(self):
        report, rows = self.material()
        for row in rows:
            if row.get('level') == 'error' and row.get('spans'):
                row['spans'][0]['file_name'] = '/rustc/8bab26f4/library/core/src/clone.rs'
        self.assertFalse(self.accepts('held_stream_accepted', report, rows))

    def test_frontend_or_warning_only_never_negative(self):
        report, rows = self.material()
        self.assertFalse(self.accepts('held_stream_accepted', report, rows[:2]))
        rows[2]['code'] = {'code': 'E0308', 'explanation': None}
        self.assertFalse(self.accepts('held_stream_accepted', report, rows))

    def test_changed_logical_site_message_or_field(self):
        report, rows = self.material()
        for mutation in ('message', 'site', 'extra'):
            changed = copy.deepcopy(rows)
            if mutation == 'site': changed[2]['spans'][0]['line_end'] += 1
            else: changed[2][mutation] = 'unreviewed'
            self.assertFalse(self.accepts('held_stream_accepted', report, changed))

    def test_duplicate_json_and_nonfinite_refused(self):
        for raw in ('{"x":1,"x":2}', '{"x":NaN}', '{"x":Infinity}'):
            with self.assertRaises(ValueError): diagnostics.parse(raw)

    def test_include_alias_is_exact_and_has_no_symlink(self):
        root = self.sources['held_stream_accepted']
        files = self.policy['cases']['held_stream_accepted']['inputs']
        alias = next(iter(diagnostics.INCLUDE_ALIASES))
        self.assertEqual(diagnostics.diagnostic_member(str(root / alias), root, files), diagnostics.BODY)
        with self.assertRaises(ValueError):
            diagnostics.diagnostic_member(str(root / 'crates/unused/../fe2o3-runtime/src/context/cached_poll_body.rs'), root, files)

    def test_qualifier_keeps_whole_crate_no_cheating_and_original_limits(self):
        from types import SimpleNamespace
        qualifier = load('cached_qualifier_controls', HERE / 'qualify-context-cached-poll-v1.py')
        source = SimpleNamespace(PROOF=Path(diagnostics.PROOF))
        self.assertEqual(qualifier.proof_command(Path('/verifier/verus'), Path('/source'), source),
                         ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '120',
                          '/verifier/verus', '--crate-type', 'lib', '--triggers-mode', 'silent',
                          '--no-cheating', '--output-json', '--error-format=json', '--num-threads', '4',
                          '/source/' + diagnostics.PROOF])

    def test_policy_requires_review_before_process_owner_import(self):
        import ast
        tree = ast.parse((HERE / 'qualify-context-cached-poll-v1.py').read_text())
        main = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'main')
        activation = [node.lineno for node in ast.walk(main) if isinstance(node, ast.Call)
                      and isinstance(node.func, ast.Attribute) and node.func.attr == 'policy_check']
        owner = [node.lineno for node in ast.walk(main) if isinstance(node, ast.Assign)
                 and any(isinstance(target, ast.Name) and target.id == 'owner' for target in node.targets)]
        self.assertEqual(len(activation), 1)
        self.assertEqual(len(owner), 1)
        self.assertLess(activation[0], owner[0])

    def test_policy_cannot_extend_named_omission(self):
        policy = copy.deepcopy(self.policy)
        policy['cases']['held_stream_accepted']['functions'].remove(
            'context_cached_poll_v1::RuntimeContextV1::unheld_stream_v1')
        with self.assertRaises(ValueError): diagnostics.policy_check(policy)
        policy = copy.deepcopy(self.policy)
        policy['cases']['held_stream_accepted']['inputs'][diagnostics.BODY]['sha256'] = '0' * 64
        with self.assertRaises(ValueError): diagnostics.policy_check(policy)


if __name__ == '__main__':
    suite = unittest.TestSuite((
        unittest.defaultTestLoader.loadTestsFromModule(inherited),
        unittest.defaultTestLoader.loadTestsFromTestCase(DiagnosticControls),
    ))
    stream = io.StringIO()
    result = unittest.TextTestRunner(stream=stream, verbosity=2).run(suite)
    if not result.wasSuccessful():
        sys.stderr.write(stream.getvalue())
        raise SystemExit(1)
    print('PASS: cached-prefix source and diagnostic controls')
