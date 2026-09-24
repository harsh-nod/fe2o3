#!/usr/bin/env python3
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('scratch_verify_test', HERE / 'verify.py')
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)

class ReceiptTests(unittest.TestCase):
    def exercise(self, mutation=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'stdout').write_bytes(b'output\n')
            (root / 'stderr').write_bytes(b'')
            row = {'exit': 0, 'error': None, 'group_absent': True, 'started_ns': 10**9, 'finished_ns': 2 * 10**9,
                   'timeout_seconds': 3, 'pid': 42, 'command': ['command'], 'environment': {'LANG': 'C'}, 'cwd': '/owned',
                   'stdout_sha256': V.sha(root / 'stdout'), 'stderr_sha256': V.sha(root / 'stderr'), 'stdin_sha256': None}
            if mutation:
                mutation(row, root)
            (root / 'receipt.json').write_text(json.dumps(row))
            return V.receipt(root, ['command'], {'LANG': 'C'}, Path('/owned'), 3)

    def test_accept_complete_receipt(self):
        self.assertEqual(self.exercise()['exit'], 0)

    def test_reject_boolean_exit(self):
        with self.assertRaises(RuntimeError):
            self.exercise(lambda row, root: row.update(exit=False))

    def test_reject_changed_stream(self):
        with self.assertRaises(RuntimeError):
            self.exercise(lambda row, root: (root / 'stderr').write_bytes(b'changed'))

    def test_reject_unexpected_stdin(self):
        with self.assertRaises(RuntimeError):
            self.exercise(lambda row, root: row.update(stdin_sha256='a' * 64))

    def test_reject_late_or_unreaped_execution(self):
        for changes in ({'finished_ns': 5 * 10**9}, {'group_absent': False}, {'error': 'late error'}, {'started_ns': True}):
            with self.subTest(changes=changes), self.assertRaises(RuntimeError):
                self.exercise(lambda row, root: row.update(changes))

    def test_reject_changed_command_environment_cwd_or_bound(self):
        for changes in ({'command': ['other']}, {'environment': {}}, {'cwd': '/other'}, {'timeout_seconds': 4}):
            with self.subTest(changes=changes), self.assertRaises(RuntimeError):
                self.exercise(lambda row, root: row.update(changes))

    def test_reject_overlapping_receipt_sequence(self):
        with self.assertRaises(RuntimeError):
            V.sequence([{'finished_ns': 3}, {'started_ns': 2}])

class ArchiveTests(unittest.TestCase):
    def exercise(self, names, expected=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'raw.tar.gz'
            with tarfile.open(archive, 'w:gz') as stream:
                for name in names:
                    member = tarfile.TarInfo(name)
                    member.size = 3
                    stream.addfile(member, io.BytesIO(b'raw'))
            index = {'archive_sha256': V.sha(archive), 'files': expected or {'logs/stdout': hashlib.sha256(b'raw').hexdigest()}}
            V.unpack(archive, index, root / 'unpacked')

    def test_accept_exact_ordinary_archive(self):
        self.exercise(['logs/stdout'])

    def test_reject_omitted_extra_duplicate_and_traversal_members(self):
        for names in ([], ['extra'], ['logs/stdout', 'logs/stdout'], ['../escape'], ['/absolute']):
            with self.subTest(names=names), self.assertRaises(RuntimeError):
                self.exercise(names)

    def test_reject_substituted_raw_bytes(self):
        with self.assertRaises(RuntimeError):
            self.exercise(['logs/stdout'], {'logs/stdout': '0' * 64})

    def test_reject_duplicate_json_keys(self):
        with self.assertRaises(RuntimeError):
            json.loads('{"exit":0,"exit":1}', object_pairs_hook=V.unique)

class ControlTests(unittest.TestCase):
    def test_exact_owned_outputs(self):
        marker = {'path': '/owned', 'commit': 'commit', 'binding_sha256': 'binding'}
        V.control_output(dict(marker), 'create', marker)
        V.control_output({'removed': '/owned'}, 'cleanup', marker)
        V.control_output({'path_absent': True, 'processes_absent': True}, 'absence', marker)

    def test_reject_false_integer_missing_and_extra_absence_claims(self):
        for value in ({'path_absent': False, 'processes_absent': False}, {'path_absent': True, 'processes_absent': False},
                      {'path_absent': 1, 'processes_absent': True}, {'path_absent': True},
                      {'path_absent': True, 'processes_absent': True, 'extra': True}):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                V.control_output(value, 'absence', {'path': '/owned'})

    def test_reject_foreign_created_or_removed_path(self):
        for name, value in (('create', {'path': '/foreign'}), ('cleanup', {'removed': '/foreign'})):
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                V.control_output(value, name, {'path': '/owned'})

    def test_exact_binary_roster_and_canonical_hashes(self):
        valid = {name: 'a' * 64 for name in ('baseline', 'candidate', 'hip', 'hsa')}
        V.binary_roster(valid)
        for value in ({key: digest for key, digest in valid.items() if key != 'hip'}, {**valid, 'extra': 'a' * 64},
                      {**valid, 'hip': 'g' * 64}, {**valid, 'hsa': True}, {**valid, 'hip': 'a' * 63}):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                V.binary_roster(value)

    def test_reject_empty_partial_or_extra_protocol_roster(self):
        valid = {name: 'a' * 64 for name in V.PROTOCOL_NAMES}
        V.protocol_roster(valid)
        for value in ({}, {key: digest for key, digest in valid.items() if key != 'campaign.py'},
                      {key: digest for key, digest in valid.items() if key != 'test_cpu_binding.py'}, {**valid, 'extra': 'a' * 64}):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                V.protocol_roster(value)

if __name__ == '__main__':
    unittest.main()
