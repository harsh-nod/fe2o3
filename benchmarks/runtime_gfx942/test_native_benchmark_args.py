#!/usr/bin/env python3

"""Host-only tests for native benchmark argument and arithmetic validation."""

from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


DIRECTORY = pathlib.Path(__file__).resolve().parent
SOURCE = DIRECTORY / "native_benchmark_args_test.cpp"
COMPARATORS = (
    "async_copy_hip.cpp",
    "async_copy_hsa.cpp",
    "async_copy_multi_hip.cpp",
    "async_copy_multi_hsa.cpp",
    "d2d_copy_hip.cpp",
    "d2d_copy_hsa.cpp",
    "xgmi_peer_hip.cpp",
    "xgmi_peer_hsa.cpp",
)


class NativeBenchmarkArgumentTests(unittest.TestCase):
    def test_strict_parsing_and_checked_arithmetic(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            executable = pathlib.Path(temporary_directory) / "native-args-test"
            subprocess.run(
                [
                    "/usr/bin/g++",
                    "-std=c++17",
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-pedantic",
                    "-I",
                    str(DIRECTORY),
                    str(SOURCE),
                    "-o",
                    str(executable),
                ],
                check=True,
            )
            subprocess.run([str(executable)], check=True)

    def test_legacy_comparators_use_checked_arguments(self) -> None:
        for name in COMPARATORS:
            with self.subTest(comparator=name):
                source = (DIRECTORY / name).read_text(encoding="utf-8")
                self.assertIn('#include "native_benchmark_args.hpp"', source)
                self.assertIn("parse_workload_shape", source)
                self.assertIn("workload.total_iterations", source)
                self.assertIn("workload.transfer_bytes", source)
                self.assertNotIn("std::atoi", source)
                self.assertNotIn("std::strtoull", source)
                self.assertNotIn("warmups + samples", source)


if __name__ == "__main__":
    unittest.main()
