#!/usr/bin/env python3
"""Mutation checks against the actual CPU evidence verifier."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import contextlib
import hashlib
import io
import json
from pathlib import Path
import shutil
import tempfile
from types import ModuleType
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "qualify.py"
RAW = SCRIPT.read_bytes()
if SCRIPT.is_symlink() or hashlib.sha256(RAW).hexdigest() != \
        "665ba3ace41859dbe02e36bfa89fe5a331c52ecad00869de9f66c72b8cbf453e":
    raise RuntimeError("authenticated qualification helper")
M = ModuleType("native_producer_cpu_tests")
M.__file__ = str(SCRIPT)
exec(compile(RAW, str(SCRIPT), "exec"), M.__dict__)


class ReplayTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="fe2o3-native-producer-replay-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / "cpu4"
        shutil.copytree(HERE / "raw/cpu4", self.output)

    def replay(self):
        with contextlib.redirect_stdout(io.StringIO()):
            M.verify(self.output)

    def test_current_source_positive(self):
        self.replay()

    def test_command_controls(self):
        path = self.output / "commands/gnu-focused/receipt.json"
        original = path.read_bytes()
        row = json.loads(original)
        for field, value in {
            "extra": 1, "command": ["true"], "environment": {}, "cwd": "/", "exit": False,
            "group_absent": False, "error": "timeout", "pid": 0, "started_ns": -1,
            "finished_ns": row["started_ns"], "timeout_seconds": 1800.0, "stdin_sha256": "0" * 64,
            "stdout_sha256": "0" * 64,
        }.items():
            with self.subTest(field=field):
                path.write_text(json.dumps(row | {field: value}))
                with self.assertRaises(RuntimeError):
                    self.replay()
        path.write_bytes(original)

    def test_missing_extra_and_aliased_artifacts(self):
        extra = self.output / "extra"
        extra.touch()
        with self.assertRaisesRegex(RuntimeError, "exact campaign tree"):
            self.replay()
        extra.unlink()
        stage = self.output / "commands/gnu-focused"
        stage.rename(self.root / "stage")
        with self.assertRaisesRegex(RuntimeError, "exact stages"):
            self.replay()
        stage.symlink_to(self.root / "stage", target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "ordinary evidence tree"):
            self.replay()

    def test_source_scope_is_exact(self):
        before = self.output / "inputs-before.json"
        after = self.output / "inputs-after.json"
        original = json.loads(before.read_bytes())
        prior = M.V.read(M.baseline("inputs-before.json"))["source"]
        changed = "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests.rs"
        for kind in ("omitted", "extra", "substituted"):
            row = json.loads(json.dumps(original))
            if kind != "extra":
                row["source"][changed] = prior[changed]
            if kind != "omitted":
                row["source"]["crates/fe2o3-runtime/src/context.rs"] = "0" * 64
            before.write_text(json.dumps(row))
            after.write_text(json.dumps(row))
            with patch.object(M.Q, "inputs", return_value=row):
                with self.assertRaisesRegex(RuntimeError, "three test-only source paths"):
                    self.replay()

    def test_hardware_tests_cannot_be_omitted_duplicated_or_promoted(self):
        path = self.output / "commands/musl-runtime/stdout"
        receipt = path.parent / "receipt.json"
        original = path.read_text()
        row = json.loads(receipt.read_bytes())
        name = sorted(M.ADDED)[0]
        line = next(line for line in original.splitlines(keepends=True) if line.startswith("test " + name + " "))
        for changed in (original.replace(line, ""), original + line,
                        original.replace(name, "foreign-hardware-test"),
                        original.replace(line, "test " + name + " ... ok\n"),
                        original.replace("22 ignored", "20 ignored")):
            path.write_text(changed)
            receipt.write_text(json.dumps(row | {"stdout_sha256": hashlib.sha256(changed.encode()).hexdigest()}))
            with self.assertRaises(RuntimeError):
                self.replay()

    def test_baseline_pin_checked_before_parse(self):
        for name in M.BASELINE:
            destination = self.root / "baseline/raw/cpu2" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text("{untrusted")
            with patch.object(M, "BASE", self.root / "baseline"):
                with self.assertRaisesRegex(RuntimeError, "qualified baseline identity"):
                    M.baseline(name)

    def test_alias_rejected_by_controller_entry(self):
        raw = self.root / "raw"
        raw.mkdir()
        alias = raw / "cpu4"
        alias.symlink_to(self.output, target_is_directory=True)
        with patch.object(M.Q, "HERE", self.root), patch.object(sys, "argv", [
            "qualify.py", "--output", str(alias), "--verify"
        ]):
            with self.assertRaisesRegex(RuntimeError, "exact new evidence directory"):
                M.Q.main()

    def test_cleanup_rejects_an_unaccounted_attempt_or_private_tree(self):
        path = HERE / "audit.py"
        audit = ModuleType("producer_cleanup_roster_test")
        audit.__file__ = str(path)
        exec(compile(path.read_bytes(), str(path), "exec"), audit.__dict__)
        packet = self.root / "packet"
        raw = packet / "raw"
        raw.mkdir(parents=True)
        for name in ("cpu1", "cpu2", "cpu3", "cpu4"):
            (raw / name).mkdir()
        for number in range(1, 4):
            (raw / f"cpu{number}-producer_launch.rs").touch()
        private = self.root / "private"
        (private / "target").mkdir(parents=True)
        with patch.object(audit, "HERE", packet), patch.object(audit.M, "PRIVATE", private):
            audit.cleanup_roster()
            (raw / "cpu5").mkdir()
            with self.assertRaisesRegex(RuntimeError, "exact attempts"):
                audit.cleanup_roster()
            (raw / "cpu5").rmdir()
            (private / "other").mkdir()
            with self.assertRaisesRegex(RuntimeError, "sole owned CPU cache"):
                audit.cleanup_roster()


if __name__ == "__main__":
    unittest.main(verbosity=2)
