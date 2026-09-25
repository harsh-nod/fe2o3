#!/usr/bin/env python3
"""Focused refusal tests for the owned proof scratch collector."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('mixed_finalize_tests', Path(__file__).with_name('finalize.py'))
F = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(F)


class FinalizerTests(unittest.TestCase):
    def collection_fixture(self, parent):
        source, output, retained = (Path(parent) / name for name in ('source', 'output', 'retained'))
        source.mkdir()
        (source / 'campaign1.lock').touch()
        for root, count in ((source / 'campaign1', 15), (output / 'commands', 4)):
            for index in range(count):
                path = root / str(index) / 'record.json'
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps({'command': ['fixture'], 'started_ns': 1, 'finished_ns': 2,
                    'process_group': 123 + index, 'status': 0, 'group_absent': True}))
        return source, output, retained

    def collect(self, source, output, retained, current=lambda: True):
        with patch.object(F, 'visibility', return_value={'scope': 'test fixture only'}):
            return F.retain_and_remove(source, retained, output, {'campaign1', 'campaign1.lock'},
                types.SimpleNamespace(group_exists=lambda _: False), current)

    def test_roster_and_links(self):
        with tempfile.TemporaryDirectory() as parent:
            root = Path(parent)
            (root / 'one').write_text('one')
            F.snapshot(root, {'one'})
            with self.assertRaisesRegex(ValueError, 'roster'):
                F.snapshot(root, set())
            os.link(root / 'one', root / 'two')
            with self.assertRaisesRegex(ValueError, 'hard-linked'):
                F.snapshot(root, {'one', 'two'})
            (root / 'two').unlink()
            (root / 'two').symlink_to(root / 'one')
            with self.assertRaises((ValueError, RuntimeError)):
                F.snapshot(root, {'one', 'two'})

    def test_tree_drift_blocks_removal(self):
        with tempfile.TemporaryDirectory() as parent:
            root = Path(parent)
            (root / 'one').write_text('one')
            before = F.snapshot(root, {'one'})
            (root / 'one').write_text('two')
            with self.assertRaises((ValueError, RuntimeError)):
                F.C.remove_owned(root, before)
            self.assertTrue(root.is_dir())

    def test_terminal_receipts(self):
        with tempfile.TemporaryDirectory() as parent:
            root = Path(parent)
            (root / 'case').mkdir()
            path = root / 'case/record.json'
            row = {'command': ['proof'], 'started_ns': 1, 'finished_ns': 2,
                   'process_group': 123, 'status': 1, 'group_absent': True}
            path.write_text(json.dumps(row))
            self.assertEqual(F.terminal_groups(root, types.SimpleNamespace(group_exists=lambda _: False)), [123])
            with self.assertRaisesRegex(ValueError, 'remains live'):
                F.terminal_groups(root, types.SimpleNamespace(group_exists=lambda _: True))
            for change in ({'status': True}, {'status': 124}, {'status': -9}, {'group_absent': False},
                           {'process_group': True}, {'exception': 'TimeoutExpired'}):
                path.write_text(json.dumps(row | change))
                with self.assertRaisesRegex(ValueError, 'receipt'):
                    F.terminal_groups(root, types.SimpleNamespace(group_exists=lambda _: False))

    def test_inventory_detects_copy_changes(self):
        with tempfile.TemporaryDirectory() as parent:
            root = Path(parent)
            (root / 'one').write_text('one')
            before = F.inventory(root)
            (root / 'one').write_text('two')
            self.assertNotEqual(F.inventory(root), before)
            (root / 'one').write_text('one')
            (root / 'extra').write_text('extra')
            self.assertNotEqual(F.inventory(root), before)

    def test_collection_retains_before_removal(self):
        with tempfile.TemporaryDirectory() as parent:
            source, output, retained = self.collection_fixture(parent)
            before = F.inventory(source)
            remove = F.shutil.rmtree
            def checked_remove(path):
                self.assertEqual(path, source)
                self.assertEqual(F.inventory(retained), before)
                self.assertTrue((output / 'cleanup-before.json').is_file())
                remove(path)
            with patch.object(F.shutil, 'rmtree', side_effect=checked_remove):
                files, _ = self.collect(source, output, retained)
            self.assertEqual(files, before)
            self.assertFalse(source.exists())
            self.assertTrue(json.loads((output / 'cleanup-after.json').read_text())['absent'])

    def test_held_lock_refuses_without_copy(self):
        with tempfile.TemporaryDirectory() as parent:
            source, output, retained = self.collection_fixture(parent)
            with (source / 'campaign1.lock').open('rb') as lock:
                F.fcntl.flock(lock, F.fcntl.LOCK_EX | F.fcntl.LOCK_NB)
                with self.assertRaises(BlockingIOError):
                    self.collect(source, output, retained)
            self.assertTrue(source.exists())
            self.assertFalse(retained.exists())

    def test_control_and_source_drift_refuse(self):
        for source_drift in (False, True):
            with tempfile.TemporaryDirectory() as parent:
                source, output, retained = self.collection_fixture(parent)
                def current():
                    if source_drift:
                        (source / 'changed').write_text('changed')
                    return source_drift
                with self.assertRaises(ValueError):
                    self.collect(source, output, retained, current)
                self.assertTrue(source.exists())
                self.assertTrue(retained.exists())
                self.assertFalse((output / 'cleanup-after.json').exists())

    def test_incomplete_removal_never_claims_absence(self):
        for partial in (False, True):
            with tempfile.TemporaryDirectory() as parent:
                source, output, retained = self.collection_fixture(parent)
                def interrupted_remove(path):
                    if partial:
                        (path / 'campaign1/0/record.json').unlink()
                        raise OSError('interrupted removal')
                with patch.object(F.shutil, 'rmtree', side_effect=interrupted_remove):
                    with self.assertRaises((OSError, ValueError)):
                        self.collect(source, output, retained)
                self.assertTrue(source.exists())
                self.assertTrue(retained.exists())
                self.assertFalse((output / 'cleanup-after.json').exists())


if __name__ == '__main__':
    stream = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(FinalizerTests)
    result = unittest.TextTestRunner(stream=stream).run(suite)
    if not result.wasSuccessful() or result.testsRun != 8:
        print(stream.getvalue(), end='')
        raise SystemExit(1)
    print('PASS: mixed-acquisition finalizer calibration (8 groups)')
