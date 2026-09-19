#!/usr/bin/env python3
"""Adversarial calibration for the peer-batch CPU evidence verifier."""

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

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "peer_batch_cpu_verify", HERE / "verify.py"
)
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)


@unittest.skipUnless(
    (HERE / "binding.json").is_file() and (HERE / "raw").is_dir(),
    "recorded peer-batch CPU packet is not prepared yet",
)
class Calibration(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="fe2o3-peer-batch-cpu-")
        self.addCleanup(self.temp.cleanup)
        self.archive = Path(self.temp.name) / "archive"
        shutil.copytree(HERE, self.archive)

    def reset_archive(self):
        shutil.rmtree(self.archive)
        shutil.copytree(HERE, self.archive)

    def reseal(self):
        path = self.archive / "SHA256SUMS"
        if path.exists():
            path.unlink()
        V.seal(self.archive, True)

    def rejected(self):
        with self.assertRaises((RuntimeError, OSError, ValueError, KeyError)):
            V.verify_bundle(self.archive)

    def write_json(self, path, value):
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")

    def change_receipt(self, name, key, value):
        path = self.archive / "raw" / name / "receipt.json"
        row = V.read(path)
        row[key] = value
        self.write_json(path, row)

    def change_stdout(self, name, transform):
        path = self.archive / "raw" / name / "stdout"
        path.write_text(transform(path.read_text()))
        self.change_receipt(
            name, "stdout_sha256", hashlib.sha256(path.read_bytes()).hexdigest()
        )

    def test_replays_the_exact_accepted_run(self):
        self.assertTrue(V.verify_bundle(self.archive)["cpu_qualification"])

    def test_binding_source_and_roster_corruption_fail_after_reseal(self):
        binding = V.read(self.archive / "binding.json")
        binding["source_base"] = "0" * 40
        binding["rosters"]["kfd"]["count"] += 1
        self.write_json(self.archive / "binding.json", binding)
        self.reseal()
        self.rejected()

    def test_receipt_command_status_environment_and_chronology_are_exact(self):
        for key, value in (
            ("command", ["false"]),
            ("exit", False),
            ("environment", {}),
            ("started_ns", 0),
        ):
            with self.subTest(key=key):
                self.reset_archive()
                self.change_receipt("cargo", key, value)
                self.reseal()
                self.rejected()

    def test_same_count_roster_substitution_is_rejected_after_reseal(self):
        self.change_stdout(
            "gnu-kfd-roster",
            lambda value: value.replace("sdma::", "changed_sdma::", 1),
        )
        self.reseal()
        self.rejected()

    def test_passing_roster_cannot_omit_or_substitute_a_test(self):
        self.change_stdout(
            "gnu-runtime",
            lambda value: value.replace(
                "kfd_backend::xgmi_batch::", "wrong_batch::", 1
            ),
        )
        self.reseal()
        self.rejected()

    def test_gnu_musl_and_feature_modes_must_have_exact_membership(self):
        self.change_stdout(
            "example-feature-off",
            lambda value: value.replace("tests::", "tests::changed_", 1),
        )
        self.reseal()
        self.rejected()

    def test_source_substitution_is_rejected_even_when_before_equals_after(self):
        for name in ("source-before", "source-after"):
            path = self.archive / "raw" / name / "stdout"
            source = V.read(path)
            source["base"] = "0" * 40
            self.write_json(path, source)
            self.change_receipt(
                name, "stdout_sha256", hashlib.sha256(path.read_bytes()).hexdigest()
            )
        self.reseal()
        self.rejected()

    def test_tool_copy_and_tool_manifest_are_jointly_bound(self):
        (self.archive / "qualify.py").write_text("raise RuntimeError('changed')\n")
        tools = V.read(self.archive / "tools.json")
        tools[V.QUALIFY] = hashlib.sha256(
            (self.archive / "qualify.py").read_bytes()
        ).hexdigest()
        self.write_json(self.archive / "tools.json", tools)
        self.reseal()
        self.rejected()

    def test_extra_missing_symlink_and_nested_seal_are_rejected(self):
        mutations = ("extra", "missing", "symlink", "nested-seal")
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.reset_archive()
                if mutation == "extra":
                    (self.archive / "extra").write_text("unexpected\n")
                elif mutation == "missing":
                    (self.archive / "README.md").unlink()
                elif mutation == "symlink":
                    path = self.archive / "raw/cargo/stdout"
                    path.unlink()
                    path.symlink_to("/dev/null")
                else:
                    (self.archive / "raw/cargo/SHA256SUMS").write_text("")
                self.rejected()

    def test_unsafe_policy_is_exact(self):
        self.change_stdout(
            "unsafe-source",
            lambda value: value.replace(
                "unsafe_source_matches_reviewed_inventory", "different_policy_test", 1
            ),
        )
        self.reseal()
        self.rejected()

    def test_prepare_is_exclusive_and_seal_cannot_be_overwritten(self):
        with self.assertRaises(RuntimeError):
            V.prepare_binding(self.archive)
        if not (self.archive / "SHA256SUMS").exists():
            V.seal(self.archive, True)
        with self.assertRaises(FileExistsError):
            V.seal(self.archive, True)


if __name__ == "__main__":
    unittest.main()
