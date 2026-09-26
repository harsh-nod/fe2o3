#!/usr/bin/env python3
"""Fail-closed controller checks; these do not invoke Verus."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('domain_checker', HERE / 'check-resource-domain.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
REPO = HERE.parents[2]


class Qualification(unittest.TestCase):
    def setUp(self):
        self.captured = {path: checker.ordinary(REPO / path) for path in checker.SOURCES}
        self.manifest = checker.ordinary(REPO / checker.MANIFEST)

    def test_reviewed_closure_and_each_source_drift(self):
        checker.source_gate(self.manifest, self.captured)
        for path in self.captured:
            with self.subTest(path=path), self.assertRaises(ValueError):
                checker.source_gate(self.manifest, self.captured | {path: self.captured[path] + b'\n'})

    def test_manifest_and_roster_drift(self):
        with self.assertRaises(ValueError):
            checker.source_gate(self.manifest + b'\n', self.captured)
        for path in self.captured:
            changed = self.captured.copy()
            del changed[path]
            with self.subTest(path=path), self.assertRaises(ValueError):
                checker.source_gate(self.manifest, changed)

    def test_positive_requires_exact_summary_and_exit(self):
        good = b'verification results:: 19 verified, 0 errors\n'
        checker.classify(0, good)
        for code, log in [(1, good), (0, b''), (0, good + good), (0, good.replace(b'19', b'18')),
                          (0, good + b'error: hidden error\n'), (0, good.replace(b'0 errors', b'1 errors'))]:
            with self.subTest(code=code, log=log), self.assertRaises(ValueError):
                checker.classify(code, log)

    def test_negative_requires_logical_failure(self):
        good = b'error: assertion failed\nverification results:: 18 verified, 1 errors\n'
        checker.classify(1, good, True)
        for code, log in [(0, good), (124, good), (1, b'error: syntax error\n'),
                          (1, good.replace(b'assertion failed', b'out of memory')), (1, good + good),
                          (1, good.replace(b'1 errors', b'0 errors')),
                          (1, good.replace(b'18 verified', b'17 verified')),
                          (1, good + b'error: out of memory\n'),
                          (1, good + b'error: syntax error\n'),
                          (1, good + b'note: resource limit exceeded\n')]:
            with self.subTest(code=code, log=log), self.assertRaises(ValueError):
                checker.classify(code, log, True)

    def test_mutants_are_exact_distinct_body_edits(self):
        mutants = checker.mutations(self.captured)
        self.assertEqual(len(mutants), 15)
        for name, path, data in mutants:
            with self.subTest(name=name), self.assertRaises(ValueError):
                checker.source_gate(self.manifest, self.captured | {path: data})

    def test_duplicate_mutation_target_and_symlink_reject(self):
        with self.assertRaises(ValueError):
            checker.replace(b'a a', 'a', 'b')
        with tempfile.TemporaryDirectory(prefix='fe2o3-r75-checker-') as directory:
            root = Path(directory)
            (root / 'actual').write_bytes(b'bytes')
            (root / 'link').symlink_to(root / 'actual')
            with self.assertRaises(ValueError):
                checker.ordinary(root / 'link')

    def test_interrupted_controller_reaps_its_child_group(self):
        with tempfile.TemporaryDirectory(prefix='fe2o3-r75-interrupt-') as directory:
            root = Path(directory)
            code = (
                'import importlib.util; from pathlib import Path; '
                f's=importlib.util.spec_from_file_location("checker", {str(HERE / "check-resource-domain.py")!r}); '
                'm=importlib.util.module_from_spec(s); s.loader.exec_module(m); m.install_handlers(); '
                f'm.execute(["/bin/sleep", "60"], Path({directory!r}), {{"PATH":"/usr/bin:/bin"}}, Path({directory!r}), "child")'
            )
            process = subprocess.Popen(['/usr/bin/python3', '-I', '-B', '-c', code],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
            child = None
            try:
                deadline = time.monotonic() + 10
                while child is None and time.monotonic() < deadline:
                    try:
                        child = json.loads((root / 'child.pid.json').read_text())['pgid']
                    except (FileNotFoundError, json.JSONDecodeError):
                        time.sleep(0.02)
                self.assertIsNotNone(child)
                process.terminate()
                process.communicate(timeout=10)
                self.assertNotEqual(process.returncode, 0)
                self.assertTrue(json.loads((root / 'child.exit.json').read_text())['interrupted'])
                with self.assertRaises(ProcessLookupError):
                    os.killpg(child, 0)
                child = None
            finally:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.communicate()
                if child is not None:
                    try:
                        os.killpg(child, signal.SIGKILL)
                    except ProcessLookupError:
                        pass


if __name__ == '__main__':
    unittest.main()
