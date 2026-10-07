"""Observer controls; no fixture below qualifies a protected service."""
import copy
import hashlib
import importlib.util
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("manager_cpu", ROOT / "scripts/qualify_native_manager_cpu.py")
q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q)


def outcome(raw=q.EXPECTED):
    return {"status": "cargo-failed", "exitCode": 98, "logComplete": True,
            "directChildReaped": True, "logBytes": len(raw), "logSha256": hashlib.sha256(raw).hexdigest()}


class PackagedManagerCPU(unittest.TestCase):
    def test_root_identity_refuses_before_output_or_subprocess(self):
        for ids in ((0, 0, 0), (1000, 0, 1000), (1000, 1000, 0), (0, 1000, 1000)):
            with mock.patch.object(q.os, "getresuid", return_value=ids), \
                 mock.patch.object(q.os, "open") as opened, self.assertRaises(ValueError):
                q.qualify(Path("/fixture/image"), "a" * 64, Path("/unused"), None)
            opened.assert_not_called()

    def test_exact_status_and_capture(self):
        q.validate_capture(outcome(), q.EXPECTED)
        for key, value in (("status", "timeout"), ("status", "log-limit"), ("exitCode", 0),
                           ("exitCode", True), ("exitCode", 125), ("exitCode", 126), ("logComplete", False),
                           ("directChildReaped", False), ("logBytes", len(q.EXPECTED) + 1),
                           ("logSha256", "0" * 64)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                q.validate_capture({**outcome(), key: value}, q.EXPECTED)
        for raw in (q.EXPECTED + b"extra", b"another refusal\n", b"x" * (q.MAX_LOG + 1)):
            with self.assertRaises(ValueError):
                q.validate_capture(outcome(raw), raw)

    def test_original_fd_path_bytes_source_and_failure_refusals(self):
        # ELF-shaped bytes are never executed by this fixture runner.
        for mutation in (None, "replace-path", "change-bytes", "source", "timeout", "log-limit"):
            with tempfile.TemporaryDirectory(prefix="fe2o3-manager-cpu-") as directory:
                root = Path(directory)
                image = root / "manager"
                raw = b"\x7fELF" + b"fixture-only" * 8
                image.write_bytes(raw)
                image.chmod(0o755)
                original_inode = image.stat().st_ino
                calls, reports = [], []
                def run(arguments, cwd, environment, log, timeout, maximum, *, executable_fd):
                    calls.append(arguments)
                    self.assertEqual(arguments, [str(image)])
                    self.assertEqual(environment, {}, "secure-start refuses any environment entry")
                    self.assertEqual(os.fstat(executable_fd).st_ino, original_inode)
                    self.assertEqual((timeout, maximum), (5, q.MAX_LOG))
                    if mutation == "replace-path":
                        replacement = root / "replacement"
                        replacement.write_bytes(raw)
                        replacement.chmod(0o755)
                        replacement.replace(image)
                    if mutation == "change-bytes":
                        image.write_bytes(raw + b"changed")
                    log.write_bytes(q.EXPECTED)
                    if mutation in ("timeout", "log-limit"):
                        return {**outcome(), "status": mutation, "logComplete": False}
                    return outcome()
                capture = SimpleNamespace(run_command=run,
                                          write_report=lambda _, value: reports.append(copy.deepcopy(value)))
                sources = [{"fixture.rs": "same"}, {"fixture.rs": "changed" if mutation == "source" else "same"}]
                with mock.patch.object(q.os, "getresuid", return_value=(1000, 1000, 1000)), \
                     mock.patch.object(q, "source_identities", side_effect=sources):
                    report = q.qualify(image, hashlib.sha256(raw).hexdigest(), root / "result", capture)
                self.assertEqual(report["packagedEntryRefusalPassed"], mutation is None)
                self.assertTrue(report["complete"])
                for key in ("joinedProtectedPhaseQualified", "rootServiceCleanupQualified", "gpuExecution"):
                    self.assertFalse(report[key])
                self.assertEqual(len(calls), 1)
                self.assertEqual(reports[-1], report)

    def test_symlink_or_pin_substitution_never_executes(self):
        for alias in (False, True):
            with tempfile.TemporaryDirectory(prefix="fe2o3-manager-refusal-") as directory:
                root = Path(directory)
                image = root / "image"
                image.write_bytes(b"\x7fELF" + b"x" * 64)
                image.chmod(0o755)
                if alias:
                    other = root / "alias"
                    other.symlink_to(image)
                    image = other
                capture = SimpleNamespace(run_command=mock.Mock(), write_report=lambda *_: None)
                with mock.patch.object(q.os, "getresuid", return_value=(1000, 1000, 1000)), \
                     mock.patch.object(q, "source_identities", return_value={}):
                    result = q.qualify(image, "f" * 64, root / "result", capture)
                self.assertFalse(result["packagedEntryRefusalPassed"])
                capture.run_command.assert_not_called()

    def test_real_capture_executes_fd_not_argv_and_reaps_timeout_and_overflow(self):
        capture = q.support()
        with tempfile.TemporaryDirectory(prefix="fe2o3-retained-exec-") as directory:
            root = Path(directory)
            fd = os.open(sys.executable, os.O_RDONLY | os.O_CLOEXEC)
            try:
                result = capture.run_command(["/not/the/executable", "-c", "print('owned fd')"],
                                             root, {}, root / "log", 5, 1024, executable_fd=fd)
                self.assertEqual(result["exitCode"], 0)
                self.assertEqual((root / "log").read_bytes(), b"owned fd\n")
                self.assertTrue(result["directChildReaped"])
                for code, timeout, maximum, expected in (("import time;time.sleep(30)", 0.05, 1024, "timeout"),
                                                         ("print('x'*10000)", 5, 32, "log-limit")):
                    result = capture.run_command(["unused", "-c", code], root, {}, root / expected,
                                                 timeout, maximum, executable_fd=fd)
                    self.assertEqual(result["status"], expected)
                    self.assertTrue(result["directChildReaped"])
                    self.assertFalse(result["logComplete"])
            finally:
                os.close(fd)

    def test_real_fd_capture_preserves_explicit_empty_environment(self):
        # A normal env binary observes the subprocess boundary only. It is not
        # the packaged manager and cannot qualify protected startup or custody.
        capture = q.support()
        with tempfile.TemporaryDirectory(prefix="fe2o3-empty-exec-env-") as directory:
            root = Path(directory)
            fd = os.open("/usr/bin/env", os.O_RDONLY | os.O_CLOEXEC)
            try:
                with mock.patch.dict(os.environ, {"FE2O3_TEST_AMBIENT_ENTRY": "must-not-inherit"}):
                    for index, (environment, expected) in enumerate((
                            ({}, b""), ({"LC_ALL": "C"}, b"LC_ALL=C\n"))):
                        log = root / f"capture-{index}"
                        result = capture.run_command(["nonempty-argv0"], root, environment,
                                                     log, 5, 1024, executable_fd=fd)
                        self.assertEqual(result["exitCode"], 0)
                        self.assertTrue(result["directChildReaped"])
                        self.assertTrue(result["logComplete"])
                        self.assertEqual(log.read_bytes(), expected)
            finally:
                os.close(fd)


if __name__ == "__main__":
    unittest.main()
