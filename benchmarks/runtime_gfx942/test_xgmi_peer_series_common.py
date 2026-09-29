#!/usr/bin/env python3
"""CPU-only qualification of the concrete HIP/HSA directional-series loop."""

from pathlib import Path
import subprocess
import tempfile
import unittest


DIRECTORY = Path(__file__).resolve().parent


class PeerSeriesCommonTests(unittest.TestCase):
    def compile_and_run(self, extra_flags):
        with tempfile.TemporaryDirectory(prefix="fe2o3-peer-series-") as folder:
            executable = Path(folder) / "peer-series-test"
            subprocess.run([
                "/usr/bin/g++", "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror",
                "-pedantic", *extra_flags, "-I", str(DIRECTORY),
                str(DIRECTORY / "xgmi_peer_series_common_test.cpp"), "-o", str(executable),
            ], check=True, timeout=60)
            result = subprocess.run([str(executable)], check=True, capture_output=True,
                                    text=True, timeout=30)
            self.assertEqual(result.stderr, "")
            self.assertEqual(result.stdout, "series control flow: pass (CPU callbacks only)\n")

    def test_exact_order_prime_warmup_sample_and_failure_callbacks(self):
        self.compile_and_run(())

    def test_undefined_behavior_sanitizer(self):
        self.compile_and_run(("-fsanitize=undefined", "-fno-sanitize-recover=all"))


if __name__ == "__main__":
    unittest.main()
