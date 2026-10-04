#!/usr/bin/env python3

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

import harness  # noqa: E402
from compare import ComparisonError, compare_cases, parse_results  # noqa: E402


def record(
    kernel: str = "fill",
    kind: str = "i32",
    seed: int = 1,
    values: tuple[int, ...] = (7,),
    left: int = 0x5A17C3E9,
    right: int = 0x2D4BE681,
) -> bytes:
    payload = "".join(f"{value:08x}" for value in values)
    return (
        f"FE2O3_DIFF_RESULT_V1\t{kernel}\t{kind}\t{seed:016x}\t{len(values)}\t"
        f"{left:08x}\t{right:08x}\t{payload}\n"
    ).encode("ascii")


class ComparatorTests(unittest.TestCase):
    def test_exact_mismatch_is_not_hidden(self) -> None:
        with self.assertRaisesRegex(ComparisonError, "exact i32 mismatch"):
            compare_cases(record(values=(7,)), record(values=(8,)))

    def test_canary_corruption_is_rejected(self) -> None:
        with self.assertRaisesRegex(ComparisonError, "canary corruption"):
            compare_cases(record(), record(left=0))

    def test_float_tolerance_nan_and_infinity_policy(self) -> None:
        reference = record(
            kernel="affine",
            kind="f32",
            values=(0x7FC12345, 0x7F800000, 0x3F800000),
            left=0x4F123456,
            right=0xCF234567,
        )
        actual = record(
            kernel="affine",
            kind="f32",
            values=(0x7FCABCDE, 0x7F800000, 0x3F800004),
            left=0x4F123456,
            right=0xCF234567,
        )
        self.assertEqual(compare_cases(reference, actual)[0]["status"], "PASS")

    def test_malformed_and_oversized_results_fail_closed(self) -> None:
        with self.assertRaises(ComparisonError):
            parse_results(b"FE2O3_DIFF_RESULT_V1\ttoo-short\n")
        with self.assertRaisesRegex(ComparisonError, "limit"):
            parse_results(b"x" * (256 * 1024 + 1))


class HarnessTests(unittest.TestCase):
    def test_generated_fixtures_use_only_the_worker_v3_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            package = harness._prepare_fixture("fill", Path(directory))
            manifest = (package / "Cargo.toml").read_text(encoding="ascii")
            source = (package / "src/main.rs").read_text(encoding="ascii")
            self.assertIn("fe2o3-host", manifest)
            self.assertNotIn("fe2o3-core", manifest)
            self.assertIn("#[kernel(", source)
            self.assertIn("typed,", source)
            self.assertNotIn("launch!", source)
            self.assertNotIn("load_module_from_file", source)

    def test_environment_validation_and_skip_semantics(self) -> None:
        settings = harness.validate_environment({})
        self.assertFalse(settings.hardware)
        phase = harness.skip_phase(1, "fe2o3-hardware", "not enabled")
        self.assertEqual(phase["status"], "SKIP")
        with self.assertRaisesRegex(harness.HarnessError, "must be unset"):
            harness.validate_environment({"FE2O3_ALLOW_GPU_SMOKE": "yes"})
        with self.assertRaisesRegex(harness.HarnessError, "explicit FE2O3_TARGET"):
            harness.validate_environment({"FE2O3_ALLOW_GPU_SMOKE": "1"})
        with self.assertRaisesRegex(harness.HarnessError, "malformed FE2O3_TARGET"):
            harness.validate_environment({"FE2O3_TARGET": "gfx942;id"})

    def test_timeout_and_failure_propagate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            timeout = harness.run_command(
                ["sh", "-c", "sleep 5"],
                cwd=Path(directory),
                timeout_seconds=0.05,
            )
            self.assertTrue(timeout.timed_out)
            self.assertFalse(timeout.succeeded)
            failure = harness.run_command(
                ["sh", "-c", "printf failure >&2; exit 7"],
                cwd=Path(directory),
                timeout_seconds=2,
            )
            self.assertEqual(failure.returncode, 7)
            self.assertEqual(failure.stderr.data, b"failure")

    def test_gpu_identity_output_has_a_larger_bounded_cap(self) -> None:
        payload_size = harness.MAX_COMMAND_OUTPUT + 1
        command = [
            sys.executable,
            "-c",
            f"import sys; sys.stdout.buffer.write(b'x' * {payload_size})",
        ]
        with tempfile.TemporaryDirectory() as directory:
            cwd = Path(directory)
            default = harness.run_command(
                command,
                cwd=cwd,
                timeout_seconds=2,
            )
            self.assertTrue(default.stdout.truncated)

            phases: list[dict[str, object]] = []
            identity = harness._checked_phase(
                phases,
                "gpu-identity",
                command,
                cwd=cwd,
                env=None,
                timeout_seconds=2,
                output_limit=harness.MAX_GPU_IDENTITY_OUTPUT,
            )
            self.assertFalse(identity.stdout.truncated)
            self.assertEqual(identity.stdout.byte_count, payload_size)
            stdout_record = phases[0]["stdout"]
            self.assertIsInstance(stdout_record, dict)
            assert isinstance(stdout_record, dict)
            self.assertEqual(stdout_record["bytes"], payload_size)

            oversized_command = [
                sys.executable,
                "-c",
                "import sys; sys.stdout.buffer.write(b'x' * "
                f"{harness.MAX_GPU_IDENTITY_OUTPUT + 1})",
            ]
            with self.assertRaisesRegex(
                harness.HarnessError, "exceeded bounded command output"
            ):
                harness._checked_phase(
                    [],
                    "gpu-identity",
                    oversized_command,
                    cwd=cwd,
                    env=None,
                    timeout_seconds=2,
                    output_limit=harness.MAX_GPU_IDENTITY_OUTPUT,
                )

    def test_artifact_is_canonical_and_bounded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "artifact.json"
            artifact = {
                "schema": harness.SCHEMA,
                "status": "FAIL",
                "authority": {"launch": False, "proof": False},
                "failure": "x" * (64 * 1024),
            }
            size, fallback = harness.write_bounded_artifact(
                path, artifact, harness.MIN_ARTIFACT_MAX
            )
            self.assertTrue(fallback)
            self.assertLessEqual(size, harness.MIN_ARTIFACT_MAX)
            parsed = json.loads(path.read_text(encoding="ascii"))
            self.assertEqual(parsed["status"], "FAIL")
            self.assertFalse(parsed["authority"]["launch"])
            first = harness.canonical_artifact_bytes({"b": 1, "a": 2})
            second = harness.canonical_artifact_bytes({"a": 2, "b": 1})
            self.assertEqual(first, second)

    def test_remote_plan_is_credential_free_and_commit_pinned(self) -> None:
        args = Namespace(
            host="mi300x",
            target="gfx942",
            checkout="/srv/fe2o3",
            commit="a" * 40,
        )
        original = sys.stdout
        try:
            from io import StringIO

            output = StringIO()
            sys.stdout = output
            self.assertEqual(harness.prepare_remote(args), 0)
        finally:
            sys.stdout = original
        command = output.getvalue()
        self.assertIn("FE2O3_EXPECT_COMMIT", command)
        self.assertIn("gfx942", command)
        self.assertNotIn("password", command.lower())
        with self.assertRaisesRegex(harness.HarnessError, "gfx942 or gfx950"):
            harness.prepare_remote(
                Namespace(
                    host="mi300x",
                    target="gfx1151",
                    checkout="/srv/fe2o3",
                    commit="a" * 40,
                )
            )

    def test_cpu_oracle_is_deterministic(self) -> None:
        compiler = next(
            (
                resolved
                for name in ["c++", "clang++", "g++"]
                if (resolved := shutil.which(name))
            ),
            None,
        )
        if compiler is None:
            self.skipTest("no C++ compiler")
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "reference"
            subprocess.run(
                [
                    compiler,
                    "-std=c++17",
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    str(SCRIPT_DIR / "reference.cpp"),
                    "-o",
                    str(executable),
                ],
                check=True,
            )
            first = subprocess.run([executable], check=True, capture_output=True).stdout
            second = subprocess.run(
                [executable], check=True, capture_output=True
            ).stdout
            self.assertEqual(first, second)
            self.assertEqual(len(parse_results(first)), 18)


if __name__ == "__main__":
    unittest.main(verbosity=2)
