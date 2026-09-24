#!/usr/bin/env python3
"""Exercise the actual replay policy on private disposable evidence copies."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import contextlib
import hashlib
import io
import json
from pathlib import Path
import shutil
import stat
import tempfile
from types import ModuleType
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "qualify.py"
RAW = SCRIPT.read_bytes()
if not stat.S_ISREG(SCRIPT.lstat().st_mode) or hashlib.sha256(RAW).hexdigest() != \
        "4637727913136890e823e6f9a475077fbef216da4502d5899fcf569e927543b8":
    raise RuntimeError("authenticated qualification helper required")
Q = ModuleType("active_producer_qualification_tests")
Q.__file__ = str(SCRIPT)
exec(compile(RAW, str(SCRIPT), "exec"), Q.__dict__)


class ReplayTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-active-replay-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / "cpu2"
        shutil.copytree(HERE / "raw/cpu2", self.output)

    def replay(self):
        with contextlib.redirect_stdout(io.StringIO()):
            Q.verify(self.output)

    def reject_bytes(self, path, data, expected=None):
        original = path.read_bytes()
        try:
            path.write_bytes(data)
            with self.assertRaisesRegex(RuntimeError, expected or ".+"):
                self.replay()
        finally:
            path.write_bytes(original)

    def test_positive_current_source_replay(self):
        self.replay()

    def test_main_rejects_aliased_root_with_the_expected_parent(self):
        raw = self.root / "raw"
        raw.mkdir()
        alias = raw / "cpu2"
        alias.symlink_to(self.output, target_is_directory=True)
        with patch.object(Q, "HERE", self.root), patch.object(sys, "argv", [
            "qualify.py", "--output", str(alias), "--verify"
        ]):
            with self.assertRaisesRegex(RuntimeError, "exact new evidence directory"):
                Q.main()

    def test_cleanup_cannot_omit_or_substitute_failed_command_receipts(self):
        script = HERE / "finalize.py"
        finalizer = ModuleType("active_producer_cleanup_tests")
        finalizer.__file__ = str(script)
        exec(compile(script.read_bytes(), str(script), "exec"), finalizer.__dict__)
        copied = self.root / "failed"
        shutil.copytree(HERE / "raw/cpu1/commands", copied)
        pins = finalizer.CPU1_RECEIPTS
        self.assertEqual(len(finalizer.terminal_receipts(copied, pins, pins)), 6)
        folder = copied / "gnu-focused"
        moved = self.root / "missing"
        folder.rename(moved)
        with self.assertRaisesRegex(RuntimeError, "exact cleanup command roster"):
            finalizer.terminal_receipts(copied, pins, pins)
        folder.symlink_to(moved, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "ordinary evidence tree"):
            finalizer.terminal_receipts(copied, pins, pins)
        folder.unlink()
        moved.rename(folder)
        (folder / "receipt.json").write_text("{}")
        with self.assertRaisesRegex(RuntimeError, "pinned original command receipt"):
            finalizer.terminal_receipts(copied, pins, pins)

    def test_helper_tamper_and_symlink_reject_before_compilation(self):
        helper = self.root / "helper.py"
        helper.write_text("raise AssertionError('must not execute')\n")
        with patch("builtins.compile") as compiler:
            with self.assertRaisesRegex(RuntimeError, "helper identity"):
                Q.authenticated_module(helper, "0" * 64, "tampered")
            compiler.assert_not_called()
        alias = self.root / "alias.py"
        alias.symlink_to(helper)
        with patch("builtins.compile") as compiler:
            with self.assertRaisesRegex(RuntimeError, "ordinary authenticated helper"):
                Q.authenticated_module(alias, Q.R.sha(helper), "alias")
            compiler.assert_not_called()

    def test_baseline_identity_precedes_parsing(self):
        for name in Q.BASELINE_SHA:
            with self.subTest(name=name):
                destination = self.root / "baseline" / "raw/campaign1" / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text("{malformed untrusted JSON")
                with patch.object(Q, "BASE", self.root / "baseline"):
                    with self.assertRaisesRegex(RuntimeError, "baseline input identity"):
                        Q.baseline(name)

    def test_source_delta_rejects_missing_extra_and_substituted_paths(self):
        path = self.output / "inputs-before.json"
        original = path.read_bytes()
        after = self.output / "inputs-after.json"
        prior = Q.V.read(Q.baseline("inputs-before.json"))["source"]
        changed = "crates/fe2o3-runtime/src/kfd_backend.rs"
        other = "crates/fe2o3-runtime/src/context.rs"
        for kind in ("missing", "extra", "substituted"):
            row = json.loads(original)
            if kind != "extra":
                row["source"][changed] = prior[changed]
            if kind != "missing":
                row["source"][other] = "0" * 64
            path.write_text(json.dumps(row))
            after.write_text(json.dumps(row))
            with patch.object(Q, "inputs", return_value=row):
                with self.assertRaisesRegex(RuntimeError, "exact two-path source delta"):
                    self.replay()
        path.write_bytes(original)
        after.write_bytes(original)

    def test_receipt_controls_dispositions_and_output_hashes(self):
        path = self.output / "commands/gnu-focused/receipt.json"
        original = json.loads(path.read_bytes())
        mutations = {
            "extra": True, "command": ["true"], "cwd": "/", "environment": {},
            "timeout_seconds": 1800.0, "stdin_sha256": "0" * 64,
            "exit": False, "error": "timeout", "group_absent": False, "pid": 0,
            "started_ns": -1, "finished_ns": original["started_ns"],
            "stdout_sha256": "0" * 64,
        }
        for field, value in mutations.items():
            with self.subTest(field=field):
                self.reject_bytes(path, json.dumps(original | {field: value}).encode())

    def test_missing_extra_and_aliased_tree_entries(self):
        extra = self.output / "unexpected"
        extra.touch()
        with self.assertRaisesRegex(RuntimeError, "exact campaign tree"):
            self.replay()
        extra.unlink()
        stage = self.output / "commands/gnu-focused"
        moved = self.root / "stage"
        stage.rename(moved)
        with self.assertRaisesRegex(RuntimeError, "exact stage roster"):
            self.replay()
        stage.symlink_to(moved, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "ordinary evidence tree"):
            self.replay()

    def changed_stdout(self, stage, change, expected=None):
        path = self.output / "commands" / stage / "stdout"
        receipt = path.parent / "receipt.json"
        original, original_receipt = path.read_bytes(), receipt.read_bytes()
        data = change(original.decode()).encode()
        try:
            row = json.loads(original_receipt)
            row["stdout_sha256"] = hashlib.sha256(data).hexdigest()
            receipt.write_text(json.dumps(row))
            self.reject_bytes(path, data, expected)
        finally:
            receipt.write_bytes(original_receipt)

    def test_named_roster_omission_duplication_substitution_and_target_drift(self):
        name = "kfd_backend::tests::producer_launch_active_three_binding_inputs_wait_without_materialization"
        line = "test " + name + " ... ok\n"
        for change in (
            lambda text: text.replace(line, ""),
            lambda text: text.replace(line, line + line),
            lambda text: text.replace(name, name + "_substituted"),
            lambda text: text.replace("35 passed;", "34 passed;"),
        ):
            self.changed_stdout("gnu-focused", change)
        self.changed_stdout("musl-runtime", lambda text: text.replace(name, name + "_foreign"))

    def test_runtime_doctest_groups_cannot_be_other_package_counts(self):
        self.changed_stdout("docs", lambda text: text.replace("4 passed;", "27 passed;")
                            .replace("42 passed;", "27 passed;"), "46 runtime doctests")


if __name__ == "__main__":
    unittest.main()
