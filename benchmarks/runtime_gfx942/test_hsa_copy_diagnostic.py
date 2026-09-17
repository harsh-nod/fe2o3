#!/usr/bin/env python3

"""Host-only policy tests; no HSA initialization or GPU work."""

from __future__ import annotations

import pathlib
import os
import subprocess
import tempfile
import unittest


DIRECTORY = pathlib.Path(__file__).resolve().parent


class HsaCopyDiagnosticTests(unittest.TestCase):
    def test_policy(self) -> None:
        with tempfile.TemporaryDirectory(prefix="fe2o3-hsa-copy-policy-") as temporary:
            executable = pathlib.Path(temporary) / "policy-test"
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
                    str(DIRECTORY / "hsa_copy_diagnostic_test.cpp"),
                    "-o",
                    str(executable),
                ],
                check=True,
            )
            result = subprocess.run(
                [str(executable)], check=True, capture_output=True, text=True
            )
            self.assertEqual(result.stdout, "HSA_COPY_DIAGNOSTIC_POLICY_OK\n")
            self.assertEqual(result.stderr, "")


class HsaCopyDiagnosticAdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        include = pathlib.Path(os.environ.get("ROCM_PATH", "/opt/rocm")) / "include"
        if not (include / "hsa/hsa_ext_amd.h").is_file():
            raise unittest.SkipTest("ROCm headers required; policy tests still run")
        cls.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-hsa-copy-mock-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.executable = pathlib.Path(cls.temporary.name) / "mock-adapter"
        subprocess.run(
            [
                "/usr/bin/g++",
                "-std=c++17",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-pedantic",
                "-isystem",
                str(include),
                str(DIRECTORY / "async_copy_hsa_pool_engine.cpp"),
                str(DIRECTORY / "hsa_copy_diagnostic_mock.cpp"),
                "-o",
                str(cls.executable),
            ],
            check=True,
        )

    def run_adapter(
        self,
        mode: str = "",
        *,
        cpu: str = "0",
        grain: str = "fine",
        engine: str = "engine0",
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                str(self.executable),
                "1",
                cpu,
                "4096",
                "1",
                "2",
                "0xab83d2ffef0d3cdf",
                grain,
                engine,
            ],
            env={**os.environ, "FE2O3_HSA_COPY_TEST_CASE": mode},
            capture_output=True,
            text=True,
            timeout=10,
        )

    def test_each_explicit_cell_and_cpu(self) -> None:
        for cpu in ("0", "1"):
            for grain in ("fine", "coarse"):
                for engine, mask in (("engine0", 1), ("engine1", 2)):
                    with self.subTest(cpu=cpu, grain=grain, engine=engine):
                        result = self.run_adapter(cpu=cpu, grain=grain, engine=engine)
                        self.assertEqual(result.returncode, 0, result.stderr)
                        rows = []
                        for line in result.stdout.splitlines():
                            fields = dict(item.split("=", 1) for item in line.split())
                            self.assertEqual(
                                fields["schema"], "fe2o3.hsa-pool-engine-diagnostic.v1"
                            )
                            rows.append(fields)
                        config = next(row for row in rows if row["record"] == "config")
                        self.assertEqual(config["cpu_index"], cpu)
                        self.assertEqual(config["cpu_agent"], str(10 + int(cpu)))
                        self.assertEqual(config["host_grain"], grain)
                        host_pool = str(
                            (10 + int(cpu)) * 100 + (2 if grain == "fine" else 4)
                        )
                        self.assertEqual(config["host_pool"], host_pool)
                        self.assertEqual(config["device_pool"], "2104")
                        allocated = [
                            line.removeprefix("MOCK allocate ")
                            for line in result.stderr.splitlines()
                            if line.startswith("MOCK allocate ")
                        ]
                        self.assertEqual(allocated, [host_pool, host_pool, "2104"])
                        self.assertEqual(config["requested_engine_mask"], str(mask))
                        self.assertEqual(config["host_aggregate_bytes"], "8192")
                        self.assertEqual(
                            config["host_release"], "signal_screlease_before_h2d_timing"
                        )
                        rounds = [row for row in rows if row["record"] == "round"]
                        self.assertEqual(
                            [row["index"] for row in rounds], ["0", "1", "2"]
                        )
                        self.assertEqual(
                            [row["phase"] for row in rounds],
                            ["warmup", "sample", "sample"],
                        )
                        for row in rounds:
                            for direction in ("h2d", "d2h"):
                                self.assertEqual(row[f"{direction}_signal"], "0")
                                self.assertGreater(int(row[f"{direction}_total_ns"]), 0)
                                self.assertEqual(
                                    int(row[f"{direction}_submit_ns"])
                                    + int(row[f"{direction}_wait_reset_ns"]),
                                    int(row[f"{direction}_total_ns"]),
                                )
                            self.assertEqual(row["checked_bytes"], "4096")
                        self.assertEqual(rows[-1]["record"], "complete")
                        self.assertEqual(rows[-1]["validated_rounds"], "3")
                        self.assertEqual(rows[-1]["measured_rounds"], "2")
                        self.assertEqual(result.stderr.count("MOCK copy "), 6)
                        self.assertEqual(result.stderr.count(f"engine={mask}\n"), 6)
                        self.assertEqual(result.stderr.count("MOCK reset\n"), 6)
                        self.assertEqual(
                            result.stderr.count("MOCK prepare_release\n"), 3
                        )
                        self.assertEqual(
                            result.stderr.count(f"MOCK access {10 + int(cpu)} 21\n"), 3
                        )
                        self.assertTrue(
                            result.stderr.endswith(
                                "MOCK destroy\nMOCK free\nMOCK free\nMOCK free\nMOCK shutdown\n"
                            )
                        )

    def test_nondefault_cpu_access_is_explicitly_granted(self) -> None:
        result = self.run_adapter("cpu_access_nondefault", cpu="1", grain="coarse")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("cpu_access=2", result.stdout)
        self.assertEqual(result.stderr.count("MOCK access 11 21\n"), 3)
        self.assertIn("record=complete", result.stdout)

    def test_failed_access_grant_prevents_copy(self) -> None:
        result = self.run_adapter("access_error")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertNotIn("MOCK copy", result.stderr)
        self.assertNotIn("record=complete", result.stdout)

    def test_output_failure_does_not_return_success(self) -> None:
        with open("/dev/full", "w") as full:
            result = subprocess.run(
                [
                    str(self.executable),
                    "1",
                    "0",
                    "4096",
                    "1",
                    "2",
                    "0xab83d2ffef0d3cdf",
                    "fine",
                    "engine0",
                ],
                env={**os.environ, "FE2O3_HSA_COPY_TEST_CASE": ""},
                stdout=full,
                stderr=subprocess.PIPE,
                text=True,
                timeout=10,
            )
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertNotIn("MOCK allocate", result.stderr)
        self.assertIn("diagnostic output failed", result.stderr)

    def test_late_output_failure_after_cleanup_is_not_success(self) -> None:
        result = self.run_adapter("late_output_error")
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(result.stderr.count("MOCK free\n"), 3)
        self.assertIn("MOCK destroy\n", result.stderr)
        self.assertIn("MOCK shutdown\n", result.stderr)
        self.assertIn("diagnostic output failed", result.stderr)
        self.assertNotIn("record=round", result.stdout)
        self.assertNotIn("record=complete", result.stdout)

    def test_invalid_cli_precedes_hsa_initialization(self) -> None:
        for argument in ("auto", "0", "1", "engine2", ""):
            with self.subTest(engine=argument):
                result = self.run_adapter(engine=argument)
                self.assertEqual(result.returncode, 2)
                self.assertNotIn("MOCK init", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_admission_rejections_do_not_allocate_or_copy(self) -> None:
        for mode in (
            "xnack",
            "frequency_zero",
            "frequency_overflow",
            "missing_gpu",
            "uuid",
            "target",
            "kernarg_pool",
            "duplicate_pool",
            "aggregate_capacity",
            "inaccessible",
            "h2d_unavailable",
            "d2h_unavailable",
        ):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertNotIn("MOCK allocate", result.stderr)
                self.assertNotIn("MOCK copy", result.stderr)
                self.assertNotIn("record=round", result.stdout)
                self.assertNotIn("record=complete", result.stdout)

    def test_unproved_completion_never_resets_or_reuses(self) -> None:
        for mode in ("submit_error", "wait_pending", "wait_negative"):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr.count("MOCK copy "), 1)
                for forbidden in (
                    "MOCK reset",
                    "MOCK destroy",
                    "MOCK free",
                    "MOCK shutdown",
                ):
                    self.assertNotIn(forbidden, result.stderr)
                self.assertNotIn("record=round", result.stdout)
                self.assertNotIn("record=complete", result.stdout)

    def test_full_buffer_and_cleanup_failures_suppress_results(self) -> None:
        for mode in (
            "bad_first",
            "bad_middle",
            "bad_last",
            "destroy_error",
            "free_error",
            "shutdown_error",
        ):
            with self.subTest(mode=mode):
                result = self.run_adapter(mode)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertNotIn("record=round", result.stdout)
                self.assertNotIn("record=complete", result.stdout)


if __name__ == "__main__":
    unittest.main()
