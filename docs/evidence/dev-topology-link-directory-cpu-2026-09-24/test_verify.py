#!/usr/bin/env python3
"""Rehashed hostile-record controls for CPU source and cleanup acceptance."""

from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class RosterTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("directory_roster_replay", HERE / "verify.py")
        self.v = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.v)

    def test_signed_rosters_include_all_existing_and_new_tests(self):
        kfd, runtime = self.v.baseline_rosters()
        self.assertEqual(len(kfd), 1561)
        self.assertEqual(len(runtime), 1385)
        self.assertEqual(sum(name.startswith("topology::") for name in kfd), 86)
        self.assertEqual(sum(status == "ignored" for status in runtime.values()), 20)
        self.assertLessEqual(self.v.FOCUSED.items(), kfd.items())

    def test_child_headers_do_not_replace_named_outcomes(self):
        self.assertEqual(self.v.named_results("running 2 tests\nrunning 1 test\ntest a ... ok\ntest b ... ignored, hardware\n"),
                         {"a": "ok", "b": "ignored"})
        for text in ("test a ... ok\ntest a ... ok\n", "test a ... FAILED\n"):
            with self.assertRaisesRegex(RuntimeError, "unique successful named tests"):
                self.v.named_results(text)


class ReplayTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("directory_cpu_replay", HERE / "verify.py")
        self.v = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.v)
        self.temp = tempfile.TemporaryDirectory(prefix="fe2o3-directory-replay-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        shutil.copytree(HERE / "raw", self.root / "raw")
        shutil.copy2(HERE / "artifacts.json", self.root / "artifacts.json")
        self.v.ROOT = self.root

    def rewrite(self, path, change):
        path = self.root / "raw" / path
        row = json.loads(path.read_bytes())
        change(row)
        path.write_text(json.dumps(row) + "\n")

    def replay(self):
        with redirect_stdout(io.StringIO()):
            self.v.main()

    def reject(self):
        (self.root / "artifacts.json").write_text(json.dumps(self.v.P.inventory(self.root / "raw")))
        with self.assertRaises((ValueError, RuntimeError, FileNotFoundError)):
            self.replay()

    def test_valid(self):
        self.replay()

    def test_boolean_exit(self):
        self.rewrite("cpu2/commands/rustc/receipt.json", lambda row: row.update(exit=False))
        self.reject()

    def test_unreaped_group(self):
        self.rewrite("cpu2/commands/gnu-focused/receipt.json", lambda row: row.update(group_absent=False))
        self.reject()

    def test_wrong_command(self):
        self.rewrite("cpu2/commands/gnu-focused/receipt.json", lambda row: row.update(command=["true"]))
        self.reject()

    def test_wrong_target(self):
        self.rewrite("cpu2/commands/rustc/receipt.json", lambda row: row["environment"].update(CARGO_TARGET_DIR="/tmp/foreign"))
        self.reject()

    def test_forged_source_in_both_brackets(self):
        for name in ("inputs-before.json", "inputs-after.json"):
            self.rewrite("cpu2/" + name, lambda row: row["source"].update({"Cargo.toml": "0" * 64}))
        self.reject()

    def test_changed_frozen_runner(self):
        path = self.root / "raw/cpu2/runner.py"
        path.write_text(path.read_text() + "\n# changed\n")
        self.reject()

    def test_relabelled_interrupted_attempt(self):
        self.rewrite("cpu1/commands/gnu-focused/receipt.json", lambda row: row.update(exit=0, error=None))
        self.reject()

    def test_foreign_cleanup(self):
        self.rewrite("cpu2-cleanup.json", lambda rows: rows[0].update(path="/tmp/foreign"))
        self.reject()

    def test_numeric_cleanup_success(self):
        self.rewrite("cpu2-cleanup.json", lambda rows: rows[0].update(absent=1))
        self.reject()

    def test_missing_stage(self):
        shutil.rmtree(self.root / "raw/cpu2/commands/clippy")
        self.reject()

    def test_zero_focused_tests_with_rehashed_output(self):
        path = self.root / "raw/cpu2/commands/gnu-focused/stdout"
        old = path.read_text()
        self.assertIn("3 passed", old)
        path.write_text(old.replace("3 passed", "0 passed"))
        self.rewrite("cpu2/commands/gnu-focused/receipt.json", lambda row: row.update(stdout_sha256=self.v.P.sha(path)))
        self.reject()

    def test_substituted_focused_test_name(self):
        path = self.root / "raw/cpu2/commands/gnu-focused/stdout"
        old = path.read_text()
        name = next(iter(self.v.FOCUSED))
        self.assertIn(name, old)
        path.write_text(old.replace(name, "substituted::test"))
        self.rewrite("cpu2/commands/gnu-focused/receipt.json", lambda row: row.update(stdout_sha256=self.v.P.sha(path)))
        self.reject()

    def test_contradictory_failed_row(self):
        path = self.root / "raw/cpu2/commands/gnu-focused/stdout"
        path.write_text(path.read_text() + "test substituted::test ... FAILED\n")
        self.rewrite("cpu2/commands/gnu-focused/receipt.json", lambda row: row.update(stdout_sha256=self.v.P.sha(path)))
        self.reject()

    def test_nonzero_full_suite_filter(self):
        path = self.root / "raw/cpu2/commands/gnu-fe2o3-kfd/stdout"
        path.write_text(path.read_text().replace("0 filtered out", "1 filtered out"))
        self.rewrite("cpu2/commands/gnu-fe2o3-kfd/receipt.json", lambda row: row.update(stdout_sha256=self.v.P.sha(path)))
        self.reject()

    def test_command_overrun(self):
        self.rewrite("cpu2/commands/after-cargo/receipt.json", lambda row: row.update(
            finished_ns=row["started_ns"] + (row["timeout_seconds"] + 16) * 10**9))
        self.reject()


if __name__ == "__main__":
    unittest.main()
