#!/usr/bin/env python3
"""Post-run verifier calibration on disposable copies, never live evidence."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run calibration with python3 -I")

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("xgmi_cpu_accept", HERE / "accept.py")
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)


class Calibration(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="fe2o3-xgmi-cpu-calibration-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "archive"
        shutil.copytree(HERE, self.root)

    def rejected(self):
        with self.assertRaises((RuntimeError, OSError, ValueError)):
            V.verify_bundle(self.root)

    def change_receipt(self, name, key, value):
        path = self.root / "raw" / name / "receipt.json"
        row = json.loads(path.read_text())
        row[key] = value
        path.write_text(json.dumps(row))

    def change_stdout(self, name, value):
        path = self.root / "raw" / name / "stdout"
        path.write_text(value)
        self.change_receipt(
            name, "stdout_sha256", hashlib.sha256(path.read_bytes()).hexdigest()
        )

    def test_exact_successful_packet(self):
        self.assertTrue(V.verify_bundle(self.root)["cpu_qualification"])

    def test_status_and_keyset_are_strict(self):
        self.change_receipt("cargo", "exit", False)
        self.rejected()
        self.change_receipt("cargo", "exit", 0)
        self.change_receipt("cargo", "unexpected", True)
        self.rejected()

    def test_receipt_order_and_overlap_are_rejected(self):
        previous = V.read(self.root / "raw/rustc/receipt.json")["finished_ns"]
        self.change_receipt("cargo", "started_ns", previous - 1)
        self.rejected()

    def test_same_count_roster_substitution_is_rejected(self):
        path = self.root / "raw/gnu-kfd-roster/stdout"
        self.change_stdout(
            "gnu-kfd-roster",
            path.read_text().replace(
                "sdma::xgmi_diagnostic", "sdma::wrong_diagnostic", 1
            ),
        )
        self.rejected()

    def test_passing_name_substitution_with_same_summary_is_rejected(self):
        path = self.root / "raw/gnu-example/stdout"
        self.change_stdout(
            "gnu-example",
            path.read_text().replace(
                "tests::percentile_uses_nearest_rank", "tests::different_test", 1
            ),
        )
        self.rejected()

    def test_same_count_source_substitution_is_rejected(self):
        for name in ("source-before", "source-after"):
            source = V.read(self.root / "raw" / name / "stdout")
            source["base"] = "0" * 40
            self.change_stdout(name, json.dumps(source))
        self.rejected()

    def test_unsafe_policy_names_ignored_status_and_unique_closure_are_pinned(self):
        output = (self.root / "raw/unsafe-source/stdout").read_text()
        V.unsafe_policy(output)
        for mutation in [
            output.replace(
                "unsafe_source_matches_reviewed_inventory", "different_test"
            ),
            output.replace(
                "refresh_reviewed_unsafe_inventory", "different_ignored_test"
            ),
            output + "test result: ok. 5 passed; 0 failed; 1 ignored;\n",
        ]:
            with self.assertRaises(RuntimeError):
                V.unsafe_policy(mutation)

    def test_extra_missing_and_nested_seal_are_rejected(self):
        extra = self.root / "extra"
        extra.mkdir()
        self.rejected()
        extra.rmdir()
        nested = self.root / "raw/cargo/SHA256SUMS"
        nested.write_text("")
        self.rejected()
        nested.unlink()
        (self.root / "README.md").unlink()
        self.rejected()

    def test_symlink_is_not_followed(self):
        path = self.root / "raw/cargo/stdout"
        path.unlink()
        path.symlink_to("/dev/null")
        self.rejected()

    def test_frozen_tool_copy_mutation_is_rejected(self):
        path = self.root / "qualify.py"
        path.write_text("raise RuntimeError('must never execute this copy')\n")
        self.rejected()

    def test_helper_authentication_precedes_module_loading(self):
        fake_root = Path(self.temp.name) / "repository"
        for name in V.TOOLS:
            path = fake_root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(V.ROOT / name, path)
        changed = fake_root / next(reversed(V.TOOLS))
        changed.write_text("raise AssertionError('untrusted helper executed')\n")
        with (
            mock.patch.object(V, "ROOT", fake_root),
            mock.patch.object(V.importlib.util, "spec_from_file_location") as loader,
        ):
            with self.assertRaises(RuntimeError):
                V.authenticated_commands()
            loader.assert_not_called()

    def test_duplicate_json_key_is_rejected(self):
        path = self.root / "raw/cargo/receipt.json"
        path.write_text(path.read_text().replace("{", '{"exit": 0,', 1))
        self.rejected()

    def test_seal_mutation_and_overwrite_are_rejected(self):
        seal = self.root / "SHA256SUMS"
        if not seal.exists():
            V.seal(self.root, True)
        V.seal(self.root, False)
        with self.assertRaises(FileExistsError):
            V.seal(self.root, True)
        (self.root / "README.md").write_text("changed\n")
        with self.assertRaises(RuntimeError):
            V.seal(self.root, False)


if __name__ == "__main__":
    unittest.main()
