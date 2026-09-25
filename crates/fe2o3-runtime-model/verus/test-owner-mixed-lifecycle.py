#!/usr/bin/env python3
"""Calibration for the lifecycle extension; never launches Verus."""
import argparse
import contextlib
import copy
import importlib.util
import io
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

PATH = Path(__file__).with_name('check-owner-mixed-lifecycle.py')
SPEC = importlib.util.spec_from_file_location('mixed_lifecycle_check', PATH)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)
REPO = PATH.resolve().parents[3]
CAMPAIGN = None


class Calibration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.old, cls.lifecycle, cls.deps, cls.old_paths, cls.paths = CHECK.previous(REPO)
        cls.baseline = {p: cls.old.git(REPO, 'show', CHECK.BASELINE + ':' + str(p)) for p in cls.paths}
        cls.candidate = {p: cls.old.ordinary(REPO, p) for p in cls.paths | {CHECK.WITNESS, CHECK.CHECK, CHECK.TEST}}

    def test_exact_delta_and_pins(self):
        CHECK.check_delta(self.candidate, self.baseline)
        self.assertEqual(len(self.paths), 459)
        for path in (CHECK.MODEL, CHECK.WITNESS, CHECK.ALIAS, CHECK.PREVIOUS, self.old.BODY):
            changed = dict(self.candidate)
            changed[path] += b'\n// substitution\n'
            with self.subTest(path=path), self.assertRaises(ValueError):
                CHECK.check_delta(changed, self.baseline)
        for path in (CHECK.WITNESS, CHECK.ROOT, CHECK.PREVIOUS):
            changed = dict(self.candidate)
            del changed[path]
            with self.assertRaises(ValueError):
                CHECK.check_delta(changed, self.baseline)
        changed = dict(self.candidate, **{'unregistered.rs': b''})
        with self.assertRaises(ValueError):
            CHECK.check_delta(changed, self.baseline)

    def test_contracts_and_canonical_root(self):
        def audit(data):
            CHECK.contracts(data, self.baseline, self.old, self.lifecycle, self.deps)
        audit(self.candidate)
        for replacement in ({}, dict(CHECK.CONTRACT_PINS, **{'unregistered': '0' * 64})):
            with mock.patch.object(CHECK, 'CONTRACT_PINS', replacement), self.assertRaises(ValueError):
                audit(self.candidate)
        for path in (CHECK.ROOT, CHECK.ALIAS):
            changed = dict(self.candidate)
            changed[path] += b'\n'
            with self.assertRaises(ValueError):
                audit(changed)
        changed = dict(self.candidate)
        changed[CHECK.WITNESS] = self.old.once(changed[CHECK.WITNESS].decode(),
            'fn lifecycle_mixed_unknown_witness_v1() -> (result: bool)\n    ensures result,',
            'fn lifecycle_mixed_unknown_witness_v1() -> (result: bool)\n    requires false,\n    ensures result,').encode()
        with self.assertRaises(ValueError):
            audit(changed)
        changed = dict(self.candidate)
        changed[CHECK.READER] = changed[CHECK.READER].replace(
            b'1 <= phase <= 13, event == lifecycle_reader_expected_event_v1(phase),',
            b'1 <= phase <= 12, event == lifecycle_reader_expected_event_v1(phase),')
        with self.assertRaises(ValueError):
            audit(changed)

    def test_mutation_roster_and_commands(self):
        rows = CHECK.mutations(self.candidate, self.old)
        self.assertEqual([r[0] for r in rows], ['stable-history', 'producer-history', 'result-answer',
            'stable-answer', 'producer-answer', 'mixed-domain', 'stable-original', 'producer-original', 'missing-atomic-event'])
        atomic = rows[-1]
        self.assertEqual(atomic, ('missing-atomic-event', CHECK.READER,
            'actual: trace.actual.push(after), model: trace.model.push(model_after), events: trace.events.push(event)',
            'actual: trace.actual.push(after), model: trace.model.push(model_after), events: trace.events',
            'production', 'lifecycle_reader_append_event_v1'))
        original = self.candidate[CHECK.READER].decode()
        mutated = self.old.once(original, atomic[2], atomic[3])
        prefix, suffix = original.split(atomic[2])
        self.assertEqual(mutated, prefix + atomic[3] + suffix)
        self.assertIn('next.events == trace.events.push(event), next.origin == trace.origin,', prefix)
        self.assertIn('next = lifecycle_reader_append_event_v1(trace, before, *actual, model_before, *model, event);',
                      self.candidate[CHECK.WITNESS].decode())
        for row in rows:
            original = self.candidate[row[1]].decode()
            self.assertNotEqual(self.old.once(original, row[2], row[3]), original)
            command = self.old.command(self.deps['legacy'], Path('/pinned/verus'), Path('/stage') / CHECK.ROOT, row)
            self.assertEqual(command[4], '600')
            self.assertEqual(command[-5:-1], ['--verify-only-module', row[4], '--verify-function', row[5]])
        positive = self.old.command(self.deps['legacy'], Path('/pinned/verus'), Path('/stage') / CHECK.ROOT, None)
        self.assertEqual(positive[4], '1200')
        self.assertNotIn('--verify-function', positive)

    def negative(self, status=1, message='assertion failed', result=None):
        report = result if result is not None else {'encountered-error': True, 'encountered-vir-error': False,
            'verified': 0, 'errors': 1, 'is-verifying-entire-crate': False}
        self.lifecycle.check_negative(self.deps['scalar'], 'calibration', status,
            {'result': report, 'diagnostics': [{'level': 'error', 'message': message}]})

    def test_logical_failure_classification(self):
        for message in ('assertion failed', 'precondition not satisfied', 'postcondition not satisfied'):
            self.negative(message=message)
        for status in (True, False, 0, 2, 124, 137, 143, -9, None, '1'):
            with self.assertRaises(ValueError):
                self.negative(status=status)
        for message in ('syntax error', 'arithmetic overflow', 'function body check: Resource limit (rlimit) exceeded',
                        'solver failed', 'aborting due to 1 previous error'):
            with self.assertRaises(ValueError):
                self.negative(message=message)
        with self.assertRaises(ValueError):
            self.lifecycle.check_negative(self.deps['scalar'], 'mixed-logical-and-resource-failure', 1, {
                'result': {'encountered-error': True, 'encountered-vir-error': False,
                           'verified': 0, 'errors': 1, 'is-verifying-entire-crate': False},
                'diagnostics': [
                    {'level': 'error', 'message': 'postcondition not satisfied'},
                    {'level': 'error', 'message': 'function body check: Resource limit (rlimit) exceeded'},
                    {'level': 'error', 'message': 'aborting due to 2 previous errors'},
                ],
            })

    def test_report_schema(self):
        valid = {'encountered-error': True, 'encountered-vir-error': False, 'verified': 0,
                 'errors': 1, 'is-verifying-entire-crate': False}
        for key, value in [('encountered-error', False), ('encountered-vir-error', True), ('errors', 0),
                           ('verified', True), ('errors', True), ('is-verifying-entire-crate', True), ('success', True)]:
            with self.assertRaises(ValueError):
                self.negative(result=dict(valid, **{key: value}))
        for key in valid:
            changed = copy.deepcopy(valid)
            del changed[key]
            with self.assertRaises(ValueError):
                self.negative(result=changed)

    def test_reject_before_import(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / CHECK.PREVIOUS
            target.parent.mkdir(parents=True)
            target.write_bytes(b'raise RuntimeError("untrusted helper executed")\n')
            with mock.patch.object(CHECK.types, 'ModuleType', side_effect=AssertionError('loader invoked')):
                with self.assertRaises(ValueError):
                    CHECK.previous(root)
            target.unlink()
            target.symlink_to(REPO / CHECK.PREVIOUS)
            with mock.patch.object(CHECK.types, 'ModuleType', side_effect=AssertionError('loader invoked')):
                with self.assertRaises(ValueError):
                    CHECK.previous(root)

    def test_stage_continuity_and_ownership(self):
        for seconds in (120, 600, 1200):
            row = {'started_ns': 1, 'finished_ns': 1 + (seconds + 15) * 1_000_000_000}
            CHECK.receipt_budget(row, seconds)
            with self.assertRaises(ValueError):
                CHECK.receipt_budget(dict(row, finished_ns=row['finished_ns'] + 1), seconds)
            for value in (True, 0, '1'):
                with self.assertRaises(ValueError):
                    CHECK.receipt_budget(dict(row, started_ns=value), seconds)
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            (stage / 'input.rs').write_text('original\n')
            expected = self.old.stage_map(stage)
            row = {'group_absent': True, 'process_group': 123}
            base = mock.Mock()
            base.group_exists.return_value = False
            self.old.stage_gate(stage, expected, row, base)
            (stage / 'input.rs').write_text('substituted\n')
            with self.assertRaises(ValueError):
                self.old.stage_gate(stage, expected, row, base)
            (stage / 'input.rs').write_text('original\n')
            for absent in (False, 1):
                with self.assertRaises(ValueError):
                    self.old.stage_gate(stage, expected, dict(row, group_absent=absent), base)
            base.group_exists.return_value = True
            with self.assertRaises(ValueError):
                self.old.stage_gate(stage, expected, row, base)
            (stage / 'linked').symlink_to(stage / 'input.rs')
            with self.assertRaises(ValueError):
                self.old.stage_map(stage)

    def test_frozen_baseline_and_candidate_scan(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            CHECK.authenticate_baseline(REPO, self.old, self.lifecycle, self.deps, self.old_paths, self.baseline, parent)
            CHECK.materialize(parent / 'candidate', self.candidate)
            CHECK.scan(parent / 'candidate', self.deps)
            mutated = parent / 'candidate' / CHECK.WITNESS
            mutated.write_text(mutated.read_text() + '\nproof fn injected() { assume(false); }\n')
            with self.assertRaises((ValueError, self.deps['policy'].ScanError)):
                CHECK.scan(parent / 'candidate', self.deps)


class CampaignCalibration(unittest.TestCase):
    def invoke(self, campaign):
        with mock.patch.object(sys, 'argv', [str(PATH), '--repo', str(REPO),
                '--output', str(campaign), '--verify']), contextlib.redirect_stdout(io.StringIO()) as output:
            CHECK.main()
        self.assertEqual(output.getvalue(), 'PASS: mixed-lifecycle campaign replay\n')

    def test_relocated_readonly_replay(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            destination = parent / 'relocated'
            shutil.copytree(CAMPAIGN, destination)
            def inventory():
                return {str(p.relative_to(destination)): CHECK.digest(p.read_bytes())
                        for p in destination.rglob('*') if p.is_file()}
            original = inventory()
            directories = [parent, destination, *[p for p in destination.rglob('*') if p.is_dir()]]
            for path in destination.rglob('*'):
                if path.is_file():
                    path.chmod(0o444)
            for path in directories:
                path.chmod(0o555)
            try:
                self.invoke(destination)
                self.assertEqual(original, inventory())
                self.assertEqual({p.name for p in parent.iterdir()}, {'relocated'})
            finally:
                for path in directories:
                    path.chmod(0o700)

    def test_rehashed_receipts(self):
        def count(value):
            value['verification-results']['verified'] = 0
        cases = [
            ('closure-after', 'record.json', lambda v: v.update(finished_ns=v['started_ns'] + 136_000_000_000), 'receipt execution budget'),
            ('positive-before', 'record.json', lambda v: v.update(status=True), 'normal terminal receipt'),
            ('positive-before', 'record.json', lambda v: v['command'].insert(-1, '--rlimit=1000'), 'exact proof command'),
            ('positive-before', 'stdout.log', count, 'exact positive count'),
        ]
        for name, filename, mutate, reason in cases:
            with self.subTest(case=reason), tempfile.TemporaryDirectory() as temporary:
                destination = Path(temporary) / 'corrupted'
                shutil.copytree(CAMPAIGN, destination)
                path = destination / name / filename
                value = json.loads(path.read_bytes())
                mutate(value)
                path.write_text(json.dumps(value) + '\n')
                accepted_path = destination / 'accepted.json'
                accepted = json.loads(accepted_path.read_bytes())
                next(row for row in accepted if row['name'] == name)['files'][filename] = CHECK.digest(path.read_bytes())
                accepted_path.write_text(json.dumps(accepted) + '\n')
                with self.assertRaisesRegex(ValueError, reason):
                    self.invoke(destination)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--campaign', type=Path, help='also test a complete recorded campaign without running Verus')
    args = parser.parse_args()
    CAMPAIGN = args.campaign.resolve(strict=True) if args.campaign else None
    stream = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(Calibration)
    if CAMPAIGN is not None:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(CampaignCalibration))
    result = unittest.TextTestRunner(stream=stream).run(suite)
    count = 8 if CAMPAIGN is None else 10
    if not result.wasSuccessful() or result.testsRun != count:
        print(stream.getvalue(), end='')
        raise SystemExit(1)
    print(f'PASS: mixed-lifecycle checker calibration ({count} groups)')
