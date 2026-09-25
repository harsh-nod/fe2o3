#!/usr/bin/env python3
"""Exercise receipt refusal and owned-process cleanup without running Cargo."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


RUNNER = Path(__file__).with_name("run.py")


class RunnerTests(unittest.TestCase):
    def exercise(self, child, failed_phase):
        spec = importlib.util.spec_from_file_location("completion_cpu_runner", RUNNER)
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        popen = subprocess.Popen
        handlers = {}
        with tempfile.TemporaryDirectory(prefix="fe2o3-completion-runner-test-") as temporary:
            directory = Path(temporary)
            output = directory / "output"

            def spawn(_command, **kwargs):
                return popen([sys.executable, "-I", "-c", child], **kwargs)

            real_signal = runner.signal.signal

            def install(signum, handler):
                handlers[signum] = real_signal(signum, handler)

            try:
                with patch.object(sys, "argv", [str(RUNNER), "--output", str(output), "--target", temporary]), \
                     patch.object(runner, "git", return_value=b""), \
                     patch.object(runner, "snapshot", return_value={}), \
                     patch.object(runner.subprocess, "Popen", side_effect=spawn), \
                     patch.object(runner.signal, "signal", side_effect=install), \
                     contextlib.redirect_stdout(io.StringIO()):
                    with self.assertRaisesRegex(RuntimeError, f"failed phase: {failed_phase}"):
                        runner.main()
            finally:
                for signum, handler in handlers.items():
                    real_signal(signum, handler)
            receipt = json.loads((output / f"{failed_phase}.json").read_text())
            self.assertIs(receipt["passed"], False)
            self.assertIs(receipt["process_group_absent"], True)
            self.assertFalse(runner.group_exists(receipt["pgid"]))
            return receipt

    def test_nonzero_is_rejected(self):
        receipt = self.exercise("raise SystemExit(7)", "runner-tests")
        self.assertEqual(receipt["returncode"], 7)

    def test_missing_tests_are_rejected(self):
        receipt = self.exercise("pass", "focused")
        self.assertIs(receipt["roster_ok"], False)

    def test_managed_signals_stop_and_reap_owned_group(self):
        receipt = self.exercise(
            "import os, signal, time; "
            "os.kill(os.getppid(), signal.SIGTERM); "
            "os.kill(os.getppid(), signal.SIGINT); time.sleep(30)", "runner-tests"
        )
        self.assertIs(receipt["cleanup_required"], True)
        self.assertIsNotNone(receipt["interruption"])
        self.assertLess(receipt["returncode"], 0)

    def test_optimized_python_is_rejected(self):
        result = subprocess.run([sys.executable, "-I", "-O", "-B", str(RUNNER), "--help"], capture_output=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"optimized Python is not supported", result.stderr)


if __name__ == "__main__":
    unittest.main()
