#!/usr/bin/env python3
"""Compile the real HIP comparator against CPU-only protocol-checking mocks."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import hip_copy_diagnostic as payload

DIRECTORY = Path(__file__).resolve().parent


class HipCopyDiagnosticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        include = Path(os.environ.get("ROCM_PATH", "/opt/rocm")) / "include"
        if not (include / "hip/hip_runtime_api.h").is_file():
            raise unittest.SkipTest("ROCm HIP headers required; no GPU is used")
        cls.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-hip-copy-mock-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.executable = Path(cls.temporary.name) / "mock-adapter"
        subprocess.run(
            [
                "/usr/bin/g++",
                "-std=c++17",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-pedantic",
                "-D__HIP_PLATFORM_AMD__",
                "-isystem",
                str(include),
                str(DIRECTORY / "async_copy_hip.cpp"),
                str(DIRECTORY / "hip_copy_diagnostic_mock.cpp"),
                "-o",
                str(cls.executable),
            ],
            check=True,
        )

    def run_adapter(self, mode="", *, arguments=None):
        if arguments is None:
            arguments = [
                "1",
                "4096",
                "1",
                "1",
                "2",
                "0xab83d2ffef0d3cdf",
                "diagnostic-copy-only",
            ]
        return subprocess.run(
            [str(self.executable), *arguments],
            env={**os.environ, "FE2O3_HIP_COPY_TEST_CASE": mode},
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_complete_round_roster_and_no_allocator_exercise(self):
        result = self.run_adapter()
        self.assertEqual(result.returncode, 0, result.stderr)
        # The mock's stderr is its call trace, not native HIP diagnostic output.
        checked = payload.validate(
            result.stdout,
            "",
            result.returncode,
            payload.Expected(
                1, 0xAB83D2FFEF0D3CDF, "gfx942:sramecc+:xnack-", 4096, 1, 2
            ),
        )
        self.assertTrue(checked["payload_valid"])
        self.assertFalse(checked["performance_accepted"])
        rows = [
            dict(field.split("=", 1) for field in line.split())
            for line in result.stdout.splitlines()
        ]
        self.assertEqual(len(rows), 5)
        self.assertTrue(
            all(
                row["schema"] == "fe2o3.hip-directional-copy-diagnostic.v1"
                for row in rows
            )
        )
        self.assertEqual(
            rows[0],
            {
                "schema": "fe2o3.hip-directional-copy-diagnostic.v1",
                "record": "config",
                "device_index": "1",
                "unique_id": "ab83d2ffef0d3cdf",
                "target": "gfx942:sramecc+:xnack-",
                "xnack": "disabled",
                "bytes": "4096",
                "depth": "1",
                "warmups": "1",
                "samples": "2",
                "host_allocation": "hipHostMallocDefault",
                "stream": "nonblocking",
                "engine": "runtime_selected",
                "allocator_benchmark": "disabled",
            },
        )
        for index, row in enumerate(rows[1:-1]):
            self.assertEqual(row["record"], "round")
            self.assertEqual(row["index"], str(index))
            self.assertEqual(row["phase"], "warmup" if index == 0 else "sample")
            self.assertEqual(row["pattern"], str((index * 67 + 1) % 251 + 1))
            self.assertEqual(row["checked_bytes"], "4096")
            self.assertGreater(int(row["h2d_total_ns"]), 0)
            self.assertGreater(int(row["d2h_total_ns"]), 0)
        self.assertEqual(
            rows[-1],
            {
                "schema": "fe2o3.hip-directional-copy-diagnostic.v1",
                "record": "complete",
                "validated_rounds": "3",
                "measured_rounds": "2",
                "allocations_released": "3",
                "streams_destroyed": "1",
            },
        )
        self.assertEqual(result.stderr.count("MOCK copy "), 6)
        self.assertEqual(result.stderr.count("MOCK wait\n"), 6)
        self.assertNotIn("MOCK pool_", result.stderr)
        self.assertTrue(
            result.stderr.endswith(
                "MOCK device_free\nMOCK host_free\nMOCK host_free\nMOCK destroy\n"
            )
        )

    def test_legacy_mode_preserves_allocator_exercise_and_single_row(self):
        result = self.run_adapter(
            arguments=["1", "4096", "1", "1", "2", "0xab83d2ffef0d3cdf"]
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(result.stdout.splitlines()), 1)
        self.assertIn("schema=fe2o3.async-copy-benchmark.v1", result.stdout)
        self.assertIn("device_pool_alloc_free_pair_ns=", result.stdout)
        self.assertEqual(result.stderr.count("MOCK pool_allocate\n"), 10000)
        self.assertEqual(result.stderr.count("MOCK pool_free\n"), 10000)

    def test_invalid_arguments_reject_before_native_selection(self):
        baseline = [
            "1",
            "4096",
            "1",
            "1",
            "2",
            "0xab83d2ffef0d3cdf",
            "diagnostic-copy-only",
        ]
        cases = [baseline + ["extra"], baseline[:-1] + ["unknown"]]
        for index, values in {
            0: ["-1"],
            1: ["0", "268435457"],
            2: ["0", "2"],
            3: ["10000", "18446744073709551615"],
            4: ["0", "10001"],
            5: ["0", "0x", "garbage"],
        }.items():
            for value in values:
                changed = baseline.copy()
                changed[index] = value
                cases.append(changed)
        for arguments in cases:
            with self.subTest(arguments=arguments):
                result = self.run_adapter(arguments=arguments)
                self.assertEqual(result.returncode, 2)
                self.assertNotIn("MOCK select", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_identity_and_target_rejection_precede_allocations(self):
        for mode in (
            "select_error",
            "uuid_error",
            "properties_error",
            "uuid",
            "target",
            "target_prefix",
            "xnack",
            "xnack_token",
            "xnack_conflict",
        ):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertNotIn("MOCK allocate", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_setup_failures_never_submit(self):
        for mode in (
            "stream_error",
            "allocation_error",
            "download_allocation_error",
            "device_allocation_error",
        ):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertNotIn("MOCK copy", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_maximum_round_count_without_warmups(self):
        result = self.run_adapter(
            arguments=[
                "1",
                "1",
                "1",
                "0",
                "10000",
                "0xab83d2ffef0d3cdf",
                "diagnostic-copy-only",
            ]
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        self.assertEqual(len(lines), 10002)
        self.assertTrue(all("phase=sample" in line for line in lines[1:-1]))
        self.assertIn("validated_rounds=10000 measured_rounds=10000", lines[-1])
        self.assertNotIn("MOCK pool_", result.stderr)

    def test_unproved_completion_never_reuses_or_releases(self):
        for mode, copies in (
            ("submit_error", 1),
            ("wait_error", 1),
            ("d2h_submit_error", 2),
            ("d2h_wait_error", 2),
        ):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr.count("MOCK copy "), copies)
                self.assertNotIn("MOCK device_free", result.stderr)
                self.assertNotIn("MOCK host_free", result.stderr)
                self.assertNotIn("MOCK destroy", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_full_buffer_corruption_prevents_results(self):
        for mode in ("corrupt_first", "corrupt_middle", "corrupt_last"):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 3, result.stderr)
                self.assertEqual(result.stderr.count("MOCK copy "), 2)
                self.assertEqual(result.stdout, "")

    def test_cleanup_failures_cannot_emit_success(self):
        for mode in (
            "device_free_error",
            "host_free_error",
            "download_free_error",
            "destroy_error",
        ):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr.count("MOCK copy "), 6)
                self.assertEqual(result.stdout, "")

    def test_late_output_failure_is_not_success(self):
        result = self.run_adapter("late_output_error")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertTrue(
            result.stderr.endswith(
                "MOCK device_free\nMOCK host_free\nMOCK host_free\nMOCK destroy\n"
            )
        )


if __name__ == "__main__":
    unittest.main()
