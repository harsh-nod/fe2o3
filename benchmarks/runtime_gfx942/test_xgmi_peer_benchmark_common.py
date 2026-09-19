#!/usr/bin/env python3
"""CPU qualification for controls shared by the HIP/HSA hot peer benchmarks."""

from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


DIRECTORY = pathlib.Path(__file__).resolve().parent


class PeerBenchmarkCommonTests(unittest.TestCase):
    def compile_and_run(self, extra_flags: tuple[str, ...]) -> None:
        with tempfile.TemporaryDirectory(prefix="fe2o3-peer-common-") as folder:
            executable = pathlib.Path(folder) / "peer-common-test"
            subprocess.run(
                [
                    "/usr/bin/g++", "-std=c++17", "-O2", "-Wall", "-Wextra",
                    "-Werror", "-pedantic", *extra_flags, "-I", str(DIRECTORY),
                    str(DIRECTORY / "xgmi_peer_benchmark_common_test.cpp"),
                    "-o", str(executable),
                ],
                check=True,
            )
            result = subprocess.run(
                [str(executable)], check=True, capture_output=True, text=True
            )
            self.assertEqual(result.stderr, "")
            self.assertEqual(
                result.stdout,
                "peer benchmark controls, guards, patterns, and lifecycle: pass\n",
            )

    def test_checked_controls_guard_mutations_and_exact_lifecycle(self) -> None:
        self.compile_and_run(())

    def test_undefined_behavior_sanitizer(self) -> None:
        self.compile_and_run(("-fsanitize=undefined", "-fno-sanitize-recover=all"))

    def test_both_comparators_use_shared_hot_lifecycle(self) -> None:
        for backend in ("hip", "hsa"):
            with self.subTest(backend=backend):
                source = (DIRECTORY / f"xgmi_peer_{backend}.cpp").read_text()
                self.assertIn('#include "xgmi_peer_benchmark_common.hpp"', source)
                self.assertIn("parse_peer_controls(", source)
                self.assertIn("run_peer_persistent_hot(", source)
                self.assertIn("fill_peer_guarded(", source)
                self.assertIn("validate_peer_guarded(", source)


if __name__ == "__main__":
    unittest.main()
