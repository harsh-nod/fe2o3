#!/usr/bin/env python3
"""Cleanup gate tests; only temporary fixture trees are changed."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "finalize.py"
M = ModuleType("mixed_finalize_tests")
M.__file__ = str(SCRIPT)
exec(compile(SCRIPT.read_bytes(), str(SCRIPT), "exec"), M.__dict__)


class CleanupGateTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="fe2o3-mixed-cleanup-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def command(self):
        folder = self.root / "commands/example"
        folder.mkdir(parents=True)
        for stream in ("stdout", "stderr"):
            (folder / stream).write_bytes(b"")
        row = {"command": ["/usr/bin/true"], "cwd": str(M.REPO), "environment": M.ENV,
               "started_ns": 1, "finished_ns": 2, "timeout_seconds": 30, "pid": 100,
               "exit": 0, "error": None, "group_absent": True, "stdin_sha256": None,
               "stdout_sha256": hashlib.sha256(b"").hexdigest(), "stderr_sha256": hashlib.sha256(b"").hexdigest()}
        (folder / "receipt.json").write_text(json.dumps(row))
        return folder, row

    def check_command(self, live=False):
        M.terminal_groups(self.root / "commands", [("example", ["/usr/bin/true"], 30)], M.ENV,
                          SimpleNamespace(group_exists=lambda _: live))

    def test_command_controls_reject_substitution_and_unbounded_receipts(self):
        folder, row = self.command()
        self.check_command()
        for key, value in {"command": ["false"], "cwd": "/", "environment": {}, "timeout_seconds": 30.0,
                           "stdin_sha256": "0" * 64, "exit": False, "group_absent": False,
                           "started_ns": -1, "finished_ns": 46 * 10**9, "pid": True}.items():
            with self.subTest(key=key):
                (folder / "receipt.json").write_text(json.dumps(row | {key: value}))
                with self.assertRaises(RuntimeError):
                    self.check_command()

    def test_live_command_group_prevents_cleanup(self):
        self.command()
        with self.assertRaisesRegex(RuntimeError, "process group still exists"):
            self.check_command(True)

    def process(self):
        proc = self.root / "proc"
        entry = proc / "123"
        (entry / "fd").mkdir(parents=True)
        private = self.root / "owned"
        private.mkdir()
        (entry / "cwd").symlink_to(self.root)
        (entry / "cmdline").write_bytes(b"unrelated\0")
        (entry / "maps").write_bytes(b"")
        (entry / "stat").write_bytes(b"123 (fixture) S " + b"0 " * 18 + b"456\n")
        return proc, entry, private

    def test_live_descriptor_and_mapping_cannot_hide_behind_unrelated_argv(self):
        proc, entry, private = self.process()
        M.private_users_absent(proc, private)
        descriptor = entry / "fd/3"
        descriptor.symlink_to(private / "target")
        with self.assertRaisesRegex(RuntimeError, "live descriptor"):
            M.private_users_absent(proc, private)
        descriptor.unlink()
        (entry / "maps").write_text("00-01 r--p 0 00:00 1 " + str(private / "object") + " (deleted)\n")
        with self.assertRaisesRegex(RuntimeError, "live mapping"):
            M.private_users_absent(proc, private)

    def test_cwd_and_command_arguments_reject_owned_users(self):
        proc, entry, private = self.process()
        (entry / "cmdline").write_bytes(str(private / "target").encode())
        with self.assertRaisesRegex(RuntimeError, "live process"):
            M.private_users_absent(proc, private)
        (entry / "cmdline").write_bytes(b"unrelated")
        (entry / "cwd").unlink()
        (entry / "cwd").symlink_to(private)
        with self.assertRaisesRegex(RuntimeError, "live process"):
            M.private_users_absent(proc, private)

    def test_sibling_prefix_and_vanished_process_do_not_hide_incomplete_live_inspection(self):
        proc, entry, private = self.process()
        (entry / "fd/3").symlink_to(str(private) + "-sibling/target")
        M.private_users_absent(proc, private)
        (entry / "cmdline").unlink()
        with self.assertRaisesRegex(RuntimeError, "incomplete inspection"):
            M.private_users_absent(proc, private)
        shutil.rmtree(entry)
        M.private_users_absent(proc, private)

    def test_one_absence_command_rejects_files_directories_and_dangling_aliases(self):
        target = self.root / "owned"
        command = M.ABSENCE[:-1] + [str(target)]
        def status():
            return subprocess.run(command, capture_output=True, timeout=5, check=False).returncode
        self.assertEqual(status(), 0)
        target.write_bytes(b"recreated")
        self.assertNotEqual(status(), 0)
        target.unlink()
        target.mkdir()
        self.assertNotEqual(status(), 0)
        target.rmdir()
        target.symlink_to(self.root / "missing")
        self.assertNotEqual(status(), 0)

    def test_unreadable_cwd_cannot_hide_readable_private_descriptors_or_mappings(self):
        proc, entry, private = self.process()
        original = M.os.readlink
        def readlink(path):
            if path == M.os.fsencode(entry / "cwd"):
                raise PermissionError(13, "fixture denial")
            return original(path)
        (entry / "fd/3").symlink_to(private / "target")
        with patch.object(M.os, "readlink", side_effect=readlink):
            with self.assertRaisesRegex(RuntimeError, "live descriptor"):
                M.private_users_absent(proc, private)
            (entry / "fd/3").unlink()
            (entry / "maps").write_text("00-01 r--p 0 00:00 1 " + str(private / "object") + "\n")
            with self.assertRaisesRegex(RuntimeError, "live mapping"):
                M.private_users_absent(proc, private)

    def test_unreadable_fields_report_identity_instead_of_claiming_global_absence(self):
        proc, entry, private = self.process()
        original = M.os.readlink
        def readlink(path):
            if path == M.os.fsencode(entry / "cwd"):
                raise PermissionError(13, "fixture denial")
            return original(path)
        with patch.object(M.os, "readlink", side_effect=readlink):
            result = M.private_users_absent(proc, private)
        self.assertEqual(result["scope"], "same-uid-best-effort-not-global-absence")
        self.assertEqual(result["uninspectable"], [{"process_identity": {
            "pid": 123, "uid": M.os.getuid(), "start_time_ticks": 456}, "field": "cwd", "errno": 13}])


if __name__ == "__main__":
    unittest.main(verbosity=2)
