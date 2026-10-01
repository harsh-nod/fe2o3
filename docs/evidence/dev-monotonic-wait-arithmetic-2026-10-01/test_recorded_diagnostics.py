#!/usr/bin/env python3
"""Portable local-file controls; no original host, compiler or solver required."""
import copy
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('recorded_wait_controls', HERE / 'check-recorded-diagnostics.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


class Controls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        diag = c.module(HERE, 'helpers/diagnostics.py')
        parser = c.module(HERE, 'helpers/producer-input-diagnostics-v1.py')
        cls.parse = staticmethod(lambda text: diag.parse(parser, text))
        cls.files = c.read_packet(HERE, cls.parse)

    def test_actual_recorded_packet_and_narrow_scope(self):
        result = c.check(HERE)
        self.assertEqual((result['negative_cases'], result['two_input_projections']), (17, 20))
        for name in ('fresh_proof_run', 'process_custody_replayed', 'cpu_qualification_replayed',
                     'std_adapter_proved', 'performance_acceptance'):
            self.assertIs(result[name], False)

    def test_relocated_packet_opens_only_bundled_input_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'packet'
            shutil.copytree(HERE, root)
            original = Path.read_bytes

            def checked(path):
                self.assertTrue(path.resolve().is_relative_to(root.resolve()))
                return original(path)

            with patch.object(Path, 'read_bytes', checked):
                self.assertEqual(c.check(root)['negative_cases'], 17)

    def test_altered_packet_or_helper_hash_rejects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'packet'
            shutil.copytree(HERE, root)
            for name in ('final.tar.gz', 'manifest.json', 'result.json', 'helpers/diagnostics.py'):
                path = root / name
                original = path.read_bytes()
                path.write_bytes(original + b'\n')
                with self.assertRaises(ValueError):
                    c.check(root)
                path.write_bytes(original)

    def test_symlink_packet_input_rejects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'packet'
            shutil.copytree(HERE, root)
            path = root / 'final.tar.gz'
            path.unlink()
            path.symlink_to(HERE / 'final.tar.gz')
            with self.assertRaises(ValueError):
                c.check(root)

    def test_missing_or_changed_projected_source_rejects(self):
        key = 'owner/prepared/backoff-lost-carry/crates/fe2o3-kfd/src/wait_arithmetic_body.rs'
        for value in (None, self.files[key] + b'\n'):
            files = dict(self.files)
            if value is None:
                del files[key]
            else:
                files[key] = value
            with patch.object(c, 'read_packet', return_value=files), self.assertRaises((KeyError, ValueError)):
                c.check(HERE)

    def test_changed_negative_result_or_diagnostic_rejects(self):
        prefix = 'owner/capture-attempt-1/commands/increment-no-progress/'
        for name in ('stdout', 'stderr'):
            files = dict(self.files)
            files[prefix + name] = b'{}\n'
            with patch.object(c, 'read_packet', return_value=files), self.assertRaises(ValueError):
                c.check(HERE)

    def test_changed_recorded_classification_rejects(self):
        key = 'owner/capture-attempt-1/increment-no-progress-result.json'
        value = copy.deepcopy(self.parse(self.files[key].decode()))
        value['observation']['calibration']['qualified_kill'] = True
        files = {**self.files, key: c.json.dumps(value).encode()}
        with patch.object(c, 'read_packet', return_value=files), self.assertRaises(ValueError):
            c.check(HERE)


if __name__ == '__main__':
    unittest.main(verbosity=2)
