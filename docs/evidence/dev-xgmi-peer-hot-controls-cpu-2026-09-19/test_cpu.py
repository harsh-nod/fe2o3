#!/usr/bin/env python3
"""Calibrate the bounded XGMI peer hot-controls CPU evidence verifier."""

import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("run with python3 -I -B")

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("peer_hot_cpu", HERE / "cpu.py")
C = importlib.util.module_from_spec(spec)
spec.loader.exec_module(C)


class CpuEvidenceTests(unittest.TestCase):
    def test_exact_stage_and_environment_contract(self):
        commands = C.commands(C.ROOT)
        self.assertEqual(tuple(row[0] for row in commands), C.STAGES)
        self.assertEqual(len(commands), 10)
        cargo = {name: command for name, command, _ in commands if "example" in name}
        self.assertEqual(
            set(cargo), {"gnu-example", "musl-example", "feature-off-example"}
        )
        for command in cargo.values():
            self.assertEqual(command[: len(C.ENV)], C.ENV)
            self.assertEqual(command.count("--example"), 1)
        self.assertIn("--all-features", cargo["gnu-example"])
        self.assertIn("x86_64-unknown-linux-musl", cargo["musl-example"])
        self.assertIn("--no-default-features", cargo["feature-off-example"])

    def test_rosters_reject_omission_substitution_and_false_success(self):
        common = (
            "\n".join(
                f"{name} (__main__.PeerBenchmarkCommonTests.{name}) ... ok"
                for name in sorted(C.COMMON_TESTS)
            )
            + "\n\nRan 3 tests in 0.001s\n\nOK\n"
        )
        C.python_tests(common, C.COMMON_TESTS)
        for changed in (
            common.replace(" ... ok", " ... FAIL", 1),
            common.replace("test_both_", "test_other_", 1),
        ):
            with self.assertRaises(RuntimeError):
                C.python_tests(changed, C.COMMON_TESTS)
        rust = "\n".join(f"test {name} ... ok" for name in sorted(C.RUST_TESTS))
        rust += "\n\ntest result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
        C.rust_tests(rust)
        with self.assertRaises(RuntimeError):
            C.rust_tests(rust.replace(" ... ok", " ... ignored", 1))

    def test_recorded_archive_and_mutations_when_available(self):
        if not (HERE / "SHA256SUMS").exists():
            self.skipTest("record first to enable archive mutation calibration")
        C.verify(HERE)
        for mutation in ("stream", "command", "extra", "missing", "symlink"):
            with (
                self.subTest(mutation=mutation),
                tempfile.TemporaryDirectory() as folder,
            ):
                copy = Path(folder) / "archive"
                shutil.copytree(HERE, copy)
                if mutation == "stream":
                    with (copy / "raw/rustc/stdout").open("ab") as target:
                        target.write(b"x")
                elif mutation == "command":
                    path = copy / "raw/rustc/receipt.json"
                    value = json.loads(path.read_text())
                    value["command"] = ["rustc", "--version"]
                    path.write_text(json.dumps(value))
                elif mutation == "extra":
                    (copy / "unexpected").write_text("x")
                elif mutation == "missing":
                    (copy / "raw/fmt/stderr").unlink()
                else:
                    path = copy / "raw/fmt/stderr"
                    path.unlink()
                    path.symlink_to("stdout")
                with self.assertRaises((RuntimeError, FileNotFoundError)):
                    C.verify(copy)


if __name__ == "__main__":
    unittest.main()
