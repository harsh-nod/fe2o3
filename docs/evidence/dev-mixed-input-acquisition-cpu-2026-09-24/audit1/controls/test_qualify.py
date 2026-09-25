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
        "682b362a11e17589c2bb3a5f306bc85e66351ad15008996a50abc35599342ecc":
    raise RuntimeError("authenticated qualification helper")
M = ModuleType("mixed_input_cpu_tests")
M.__file__ = str(SCRIPT)
exec(compile(RAW, str(SCRIPT), "exec"), M.__dict__)


class ReplayTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="fe2o3-mixed-input-replay-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / "cpu1"
        shutil.copytree(HERE / "raw/cpu1", self.output)

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
        changed = "crates/fe2o3-runtime/src/context/versions/submissions.rs"
        for kind in ("omitted", "extra", "substituted"):
            row = json.loads(json.dumps(original))
            if kind != "extra":
                row["source"][changed] = prior[changed]
            if kind != "omitted":
                row["source"]["crates/fe2o3-runtime/src/context.rs"] = "0" * 64
            before.write_text(json.dumps(row))
            after.write_text(json.dumps(row))
            with patch.object(M.Q, "inputs", return_value=row):
                with self.assertRaisesRegex(RuntimeError, "exact thirteen-path source delta"):
                    self.replay()

    def test_runtime_roster_cannot_be_omitted_duplicated_or_substituted(self):
        path = self.output / "commands/musl-runtime/stdout"
        receipt = path.parent / "receipt.json"
        original = path.read_text()
        row = json.loads(receipt.read_bytes())
        name = sorted(M.ADDED)[0]
        line = next(line for line in original.splitlines(keepends=True) if line.startswith("test " + name + " "))
        for changed in (original.replace(line, ""), original + line,
                        original.replace(name, "foreign-hardware-test"),
                        original.replace(line, "test " + name + " ... ignored\n"),
                        original.replace("22 ignored", "20 ignored")):
            path.write_text(changed)
            receipt.write_text(json.dumps(row | {"stdout_sha256": hashlib.sha256(changed.encode()).hexdigest()}))
            with self.assertRaises(RuntimeError):
                self.replay()

    def test_baseline_pin_checked_before_parse(self):
        for name in M.BASELINE:
            destination = self.root / "baseline" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text("{untrusted")
            with patch.object(M, "BASE", self.root / "baseline"):
                with self.assertRaisesRegex(RuntimeError, "qualified baseline identity"):
                    M.baseline(name)

    def test_alias_rejected_by_controller_entry(self):
        raw = self.root / "raw"
        raw.mkdir()
        alias = raw / "cpu1"
        alias.symlink_to(self.output, target_is_directory=True)
        with patch.object(M.Q, "HERE", self.root), patch.object(sys, "argv", [
            "qualify.py", "--output", str(alias), "--verify"
        ]):
            with self.assertRaisesRegex(RuntimeError, "exact new evidence directory"):
                M.Q.main()

    def test_model_roster_keeps_existing_ignores_and_all_new_tests(self):
        path = self.output / "commands/musl-model/stdout"
        receipt = path.parent / "receipt.json"
        original = path.read_text()
        row = json.loads(receipt.read_bytes())
        name = sorted(M.MODEL_ADDED)[-1]
        line = next(line for line in original.splitlines(keepends=True) if line.startswith("test " + name + " "))
        for changed in (original.replace(line, ""), original + line,
                        original.replace(name, "unrelated-model-test"),
                        original.replace(line, "test " + name + " ... ignored\n"),
                        original.replace("18 ignored", "0 ignored")):
            path.write_text(changed)
            receipt.write_text(json.dumps(row | {"stdout_sha256": hashlib.sha256(changed.encode()).hexdigest()}))
            with self.assertRaises(RuntimeError):
                self.replay()


if __name__ == "__main__":
    unittest.main(verbosity=2)
