#!/usr/bin/env python3
"""Sealed-packet and genuine relocated-campaign replay controls."""
import argparse
import contextlib
import importlib.util
import io
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('mixed_packet_audit', Path(__file__).with_name('audit.py'))
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)
REPO = Path(__file__).resolve().parents[3]
RAW = None
PACKET = None


def dump(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def reseal(root):
    raw = {str(p.relative_to(root)): AUDIT.sha(p.read_bytes()) for p in sorted((root / 'raw').rglob('*')) if p.is_file()}
    dump(root / 'artifacts.json', {'schema': 1, 'files': raw})
    paths = [p for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'SHA256SUMS']
    (root / 'SHA256SUMS').write_text(''.join(AUDIT.sha(p.read_bytes()) + '  ' + str(p.relative_to(root)) + '\n' for p in paths))


class PacketCalibration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.checker = AUDIT.load_checker(REPO)

    def invoke(self, raw):
        saved = sys.argv
        try:
            sys.argv = [str(REPO / AUDIT.CHECK), '--repo', str(REPO), '--output', str(raw), '--verify']
            with contextlib.redirect_stdout(io.StringIO()) as stream:
                self.checker.main()
            self.assertEqual(stream.getvalue(), 'PASS: independent mixed-acquisition campaign replay\n')
        finally:
            sys.argv = saved

    def test_seal_closure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'raw').mkdir()
            (root / 'raw/log').write_bytes(b'original\n')
            (root / 'README.md').write_text('fixture\n')
            reseal(root)
            AUDIT.check_seal(root)
            (root / 'raw/log').write_bytes(b'changed\n')
            with self.assertRaisesRegex(ValueError, 'sealed bytes'):
                AUDIT.check_seal(root)
            (root / 'raw/log').write_bytes(b'original\n')
            (root / 'extra').write_bytes(b'extra')
            with self.assertRaisesRegex(ValueError, 'roster'):
                AUDIT.check_seal(root)
            (root / 'extra').unlink()
            seal = (root / 'SHA256SUMS').read_bytes()
            (root / 'SHA256SUMS').write_bytes(seal + seal.splitlines(keepends=True)[0])
            with self.assertRaisesRegex(ValueError, 'unique'):
                AUDIT.check_seal(root)
            (root / 'SHA256SUMS').write_bytes(seal)
            (root / 'raw/linked').symlink_to(root / 'raw/log')
            with self.assertRaisesRegex(ValueError, 'ordinary packet tree'):
                AUDIT.check_seal(root)

    def test_raw_manifest(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'raw').mkdir()
            (root / 'raw/log').write_bytes(b'log')
            reseal(root)
            for changed in ({'schema': True, 'files': {}}, {'schema': 1, 'files': {}}):
                dump(root / 'artifacts.json', changed)
                paths = [p for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'SHA256SUMS']
                (root / 'SHA256SUMS').write_text(''.join(AUDIT.sha(p.read_bytes()) + '  ' + str(p.relative_to(root)) + '\n' for p in paths))
                with self.assertRaises(ValueError):
                    AUDIT.check_seal(root)

    def test_relocated_readonly_replay(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            raw = parent / 'relocated'
            shutil.copytree(RAW, raw)
            before = {str(p.relative_to(raw)): AUDIT.sha(p.read_bytes()) for p in raw.rglob('*') if p.is_file()}
            directories = [raw, *[p for p in raw.rglob('*') if p.is_dir()]]
            for path in raw.rglob('*'):
                if path.is_file():
                    path.chmod(0o444)
            for path in directories:
                path.chmod(0o555)
            parent.chmod(0o555)
            try:
                self.invoke(raw)
                self.assertEqual({str(p.relative_to(raw)): AUDIT.sha(p.read_bytes()) for p in raw.rglob('*') if p.is_file()}, before)
                self.assertEqual({p.name for p in parent.iterdir()}, {'relocated'})
            finally:
                parent.chmod(0o700)
                for path in directories:
                    path.chmod(0o700)

    def changed_case(self, mutate, pattern, exception=ValueError):
        with tempfile.TemporaryDirectory() as temporary:
            raw = Path(temporary) / 'copy'
            shutil.copytree(RAW, raw)
            mutate(raw)
            with self.assertRaisesRegex(exception, pattern):
                self.invoke(raw)

    def test_replay_identities(self):
        def change(raw, key, value):
            source = AUDIT.unique_json((raw / 'source.json').read_bytes())
            source[key] = value
            dump(raw / 'source.json', source)
        self.changed_case(lambda r: change(r, 'commit', 'not-a-commit'), 'recorded source commit')
        self.changed_case(lambda r: change(r, 'inputs', {}), 'exact resume source identity')
        self.changed_case(lambda r: change(r, 'baseline', '0' * 40), 'exact resume source identity')
        self.changed_case(lambda r: change(r, 'recorded_output', 'relative'), 'absolute recorded path')
        self.changed_case(lambda r: change(r, 'recorded_output', '/different/recorded/output'), 'owned proof stage')

    def test_replay_records_and_summaries(self):
        def change_record(raw, name, filename, change):
            path = raw / name / filename
            value = AUDIT.unique_json(path.read_bytes())
            change(value)
            dump(path, value)
            accepted = AUDIT.unique_json((raw / 'accepted.json').read_bytes())
            next(row for row in accepted if row['name'] == name)['files'][filename] = AUDIT.sha(path.read_bytes())
            dump(raw / 'accepted.json', accepted)
        self.changed_case(lambda r: change_record(r, 'positive-before', 'record.json', lambda v: v.update(status=True)), 'normal terminal receipt')
        self.changed_case(lambda r: change_record(r, 'positive-before', 'record.json', lambda v: v.update(group_absent=False)), 'normal terminal receipt')
        self.changed_case(lambda r: change_record(r, 'positive-before', 'record.json', lambda v: v.update(status=1)), 'clean whole-root positive')
        self.changed_case(lambda r: change_record(r, 'positive-before', 'stdout.log',
            lambda v: v['verification-results'].update(verified=0)), 'exact whole-root count')
        self.changed_case(lambda r: dump(r / 'results.json', {}), 'existing final summary')
        self.changed_case(lambda r: dump(r / 'inputs-after.json', {}), 'existing final summary')
        self.changed_case(lambda r: dump(r / 'accepted.json', []), 'unaccepted or missing campaign records')
        self.changed_case(lambda r: (r / 'extra').write_text('extra'), 'unaccepted or missing campaign records')
        self.changed_case(lambda r: (r / 'positive-before/stdout.log').unlink(), 'incomplete or unexpected existing case')


class CleanupEvidenceCalibration(unittest.TestCase):
    def test_finalization_corruptions(self):
        AUDIT.check_finalization(REPO, PACKET)
        def corrupt(relative, update, message):
            with tempfile.TemporaryDirectory() as temporary:
                packet = Path(temporary) / 'packet'
                shutil.copytree(PACKET, packet)
                marker = AUDIT.unique_json((packet / 'finalization.json').read_bytes())
                path = packet / marker['audit'] / relative
                data = AUDIT.unique_json(path.read_bytes())
                update(data)
                dump(path, data)
                with self.assertRaisesRegex(ValueError, message):
                    AUDIT.check_finalization(REPO, packet)
        for change in ({'status': True}, {'group_absent': False}, {'command': ['substituted']}, {'finished_ns': 1}):
            corrupt('commands/replay/record.json', lambda v: v.update(change), 'receipt|command')
        corrupt('inputs-after.json', lambda v: v.clear(), 'control bracket')
        corrupt('cleanup-source.json', lambda v: v.clear(), 'scratch inventory')
        corrupt('cleanup-after.json', lambda v: v.update(absent=False), 'terminal cleanup')
        corrupt('cleanup-before.json', lambda v: v.update(retained_files={}), 'retention')
        corrupt('process-visibility-final.json', lambda v: v.update(observed_private_references=1), 'visibility')
        corrupt('process-visibility-final.json', lambda v: v.pop('uninspectable'), 'visibility')
        corrupt('process-visibility-final.json', lambda v: v.update(uninspectable_count=True), 'visibility')
        corrupt('process-visibility-final.json', lambda v: v.update(uninspectable=[{}], uninspectable_count=1), 'field schema')
        corrupt('process-visibility-final.json', lambda v: v.update(collector_reference='global absence'), 'visibility')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--raw', type=Path, required=True)
    parser.add_argument('--finalization-packet', type=Path)
    args = parser.parse_args()
    RAW = args.raw.resolve(strict=True)
    PACKET = args.finalization_packet.resolve(strict=True) if args.finalization_packet else None
    stream = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(PacketCalibration)
    if PACKET is not None:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(CleanupEvidenceCalibration))
    result = unittest.TextTestRunner(stream=stream).run(suite)
    expected = 5 if PACKET is None else 6
    if not result.wasSuccessful() or result.testsRun != expected:
        print(stream.getvalue(), end='')
        raise SystemExit(1)
    print(f'PASS: mixed-acquisition packet calibration ({expected} groups)')
