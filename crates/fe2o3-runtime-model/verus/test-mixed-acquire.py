#!/usr/bin/env python3
"""Calibration for the mixed-acquisition extension; no solver subprocesses."""
import copy
import importlib.util
import io
import tempfile
import unittest
from unittest import mock
from pathlib import Path

PATH = Path(__file__).with_name('check-mixed-acquire.py')
SPEC = importlib.util.spec_from_file_location('mixed_check', PATH)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)
REPO = PATH.resolve().parents[3]


class Calibration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.lifecycle, cls.deps, cls.paths = CHECK.inherited(REPO)

    def negative(self, status=1, result=None, messages=None):
        observed = {'result': result if result is not None else {
            'encountered-error': True, 'encountered-vir-error': False, 'verified': 0,
            'errors': 1, 'is-verifying-entire-crate': False},
            'diagnostics': [{'level': 'error', 'message': m} for m in (
                ['assertion failed', 'aborting due to 1 previous error'] if messages is None else messages)]}
        self.lifecycle.check_negative(self.deps['scalar'], 'calibration', status, observed)

    def test_valid_logical_negatives(self):
        for message in ('assertion failed', 'postcondition not satisfied', 'precondition not satisfied',
                        'invariant not satisfied before loop', 'invariant not satisfied at end of loop body'):
            self.negative(messages=[message])

    def test_rejected_statuses(self):
        for status in (True, False, 0, 2, 124, 137, 143, -9, -15, None, '1'):
            with self.subTest(status=status), self.assertRaises(ValueError):
                self.negative(status=status)

    def test_rejected_diagnostics(self):
        for message in ('syntax error', 'expected expression', 'arithmetic overflow',
                        'function body check: Resource limit (rlimit) exceeded',
                        'internal error', 'solver failed', 'aborting due to 1 previous error'):
            with self.subTest(message=message), self.assertRaises(ValueError):
                self.negative(messages=[message])
        with self.assertRaises(ValueError):
            self.negative(messages=['assertion failed', 'syntax error'])
        with self.assertRaises(ValueError):
            self.negative(messages=[])

    def test_rejected_reports(self):
        valid = {'encountered-error': True, 'encountered-vir-error': False, 'verified': 0,
                 'errors': 1, 'is-verifying-entire-crate': False}
        for key, value in [('encountered-error', False), ('encountered-vir-error', True),
                           ('errors', 0), ('errors', True), ('verified', True), ('verified', -1),
                           ('is-verifying-entire-crate', True), ('success', True)]:
            changed = dict(valid, **{key: value})
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                self.negative(result=changed)
        for key in valid:
            changed = copy.deepcopy(valid)
            del changed[key]
            with self.assertRaises(ValueError):
                self.negative(result=changed)

    def test_unique_mutations_and_commands(self):
        rows = CHECK.mutations(REPO)
        self.assertEqual([r[0] for r in rows], ['stable-before-preflight', 'combined-budget', 'pending-preflight',
            'stable-commit', 'producer-commit', 'early-error-frame', 'pending-headroom',
            'bridge-actual-executor', 'bridge-logical-executor'])
        for row in rows:
            original = (REPO / row[1]).read_text()
            self.assertNotEqual(CHECK.once(original, row[2], row[3]), original)
            command = CHECK.command(self.deps['legacy'], Path('/pinned/verus'), Path('/stage') / CHECK.ROOT, row)
            self.assertEqual(command[4], '600')
            self.assertEqual(command[-5:-1], ['--verify-only-module', row[4], '--verify-function', row[5]])
        positive = CHECK.command(self.deps['legacy'], Path('/pinned/verus'), Path('/stage') / CHECK.ROOT, None)
        self.assertEqual(positive[4], '1200')
        self.assertNotIn('--verify-function', positive)
        for original in ('missing', 'anchor anchor'):
            with self.assertRaises(ValueError):
                CHECK.once(original, 'anchor', 'replacement')

    def test_root_and_independence(self):
        CHECK.root_contract(REPO)
        source = (REPO / CHECK.MODEL).read_text()
        CHECK.independent_model(source, self.deps)
        for forbidden in ('production::mixed_acquire_relation_v1()', 'producer_represents(actual, model)',
                          'include!("production.rs")'):
            with self.subTest(forbidden=forbidden), self.assertRaises((ValueError, self.deps['policy'].ScanError)):
                CHECK.independent_model(source.rstrip()[:-1] + '\n' + forbidden + '\n}\n', self.deps)

    def test_ordinary_inputs_and_stage_map(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'normal').write_bytes(b'original')
            before = CHECK.stage_map(root)
            self.assertEqual(CHECK.ordinary(root, Path('normal')), b'original')
            (root / 'normal').write_bytes(b'changed')
            self.assertNotEqual(CHECK.stage_map(root), before)
            (root / 'linked').symlink_to(root / 'normal')
            with self.assertRaises(ValueError):
                CHECK.stage_map(root)
            for path in (Path('linked'), Path('../normal'), root / 'normal'):
                with self.assertRaises(ValueError):
                    CHECK.ordinary(root, path)

    def test_new_pins_and_inherited_closure(self):
        self.assertEqual(len(self.paths), 449)
        self.assertEqual(set(CHECK.NEW_PINS), CHECK.NEW)
        for path, pin in CHECK.NEW_PINS.items():
            self.assertEqual(CHECK.sha(CHECK.ordinary(REPO, path)), pin)
        self.assertEqual(CHECK.sha(CHECK.ordinary(REPO, CHECK.OLD_CHECK)), CHECK.OLD_SHA)

    def test_reject_before_loading(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / CHECK.OLD_CHECK
            target.parent.mkdir(parents=True)
            target.write_bytes(b'raise RuntimeError("untrusted helper executed")\n')
            with mock.patch.object(CHECK.types, 'ModuleType', side_effect=AssertionError('loader invoked')):
                with self.assertRaises(ValueError):
                    CHECK.inherited(root)
            target.unlink()
            target.symlink_to(REPO / CHECK.OLD_CHECK)
            with mock.patch.object(CHECK.types, 'ModuleType', side_effect=AssertionError('loader invoked')):
                with self.assertRaises(ValueError):
                    CHECK.inherited(root)

    def test_current_source_authentication(self):
        historical = {p: CHECK.git(REPO, 'show', CHECK.HISTORICAL + ':' + str(p)) for p in self.paths}
        baseline = {p: CHECK.git(REPO, 'show', CHECK.BASELINE + ':' + str(p)) for p in self.paths | CHECK.PRIOR_ADDITIONS}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for path in self.paths | CHECK.PRIOR_ADDITIONS | CHECK.NEW:
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(CHECK.ordinary(REPO, path))
            def authenticate():
                CHECK.authenticate_current(root, self.deps, self.paths, historical, baseline)
            authenticate()
            for path in (CHECK.MODEL, CHECK.ROOT, CHECK.BODY, next(iter(CHECK.PRIOR_CHANGES)), CHECK.REGRESSION,
                         Path('crates/fe2o3-runtime/src/context.rs')):
                original = (root / path).read_bytes()
                (root / path).write_bytes(original + b'\n// substituted\n')
                with self.subTest(path=path), self.assertRaises(ValueError):
                    authenticate()
                (root / path).write_bytes(original)
            for path in (CHECK.ROOT, CHECK.MODEL):
                original = (root / path).read_bytes()
                pin = CHECK.NEW_PINS[path]
                changed = original + b'\n// rehashed root drift\n' if path == CHECK.ROOT else original.replace(
                    b'    let ghost before = *contents;', b'    let ghost before = producer_represents(*contents, *contents);', 1)
                (root / path).write_bytes(changed)
                CHECK.NEW_PINS[path] = CHECK.sha(changed)
                try:
                    with self.subTest(rehashed=path), self.assertRaises(ValueError):
                        authenticate()
                finally:
                    CHECK.NEW_PINS[path] = pin
                    (root / path).write_bytes(original)
            extra = root / CHECK.CRATE / 'src/unregistered.rs'
            extra.write_text('// extra runtime input\n')
            with self.assertRaises(ValueError):
                authenticate()
            extra.unlink()
            extra.symlink_to(root / CHECK.BODY)
            with self.assertRaises(ValueError):
                authenticate()
            extra.unlink()
            authenticate()

    def test_stage_gate_precedes_acceptance(self):
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            (stage / 'input.rs').write_text('original\n')
            expected = CHECK.stage_map(stage)
            row = {'group_absent': True, 'process_group': 123}
            base = mock.Mock()
            base.group_exists.return_value = False
            CHECK.stage_gate(stage, expected, row, base)
            (stage / 'input.rs').write_text('substituted\n')
            with self.assertRaises(ValueError):
                CHECK.stage_gate(stage, expected, row, base)
            (stage / 'input.rs').write_text('original\n')
            for absent in (False, 1):
                with self.assertRaises(ValueError):
                    CHECK.stage_gate(stage, expected, dict(row, group_absent=absent), base)
            base.group_exists.return_value = True
            with self.assertRaises(ValueError):
                CHECK.stage_gate(stage, expected, row, base)


if __name__ == '__main__':
    stream = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(Calibration)
    result = unittest.TextTestRunner(stream=stream).run(suite)
    if not result.wasSuccessful() or result.testsRun != 11:
        print(stream.getvalue(), end='')
        raise SystemExit(1)
    print('PASS: mixed-acquisition checker calibration (11 groups)')
