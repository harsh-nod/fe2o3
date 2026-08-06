#!/usr/bin/env python3

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parents[1]
MOCK_BUILD = Path(__file__).resolve().with_name("mock_build.py")
sys.path.insert(0, str(SCRIPT_DIR))

import reproduce  # noqa: E402


class ReproducibilityCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.work = self.root / "work"
        self.source.mkdir()
        self.work.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_cli(
        self, *arguments: str, environment: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(SCRIPT_DIR / "reproduce.py"), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
            check=False,
        )

    def run_arguments(self, target: str, mode: str = "stable") -> list[str]:
        return [
            "run",
            "--target",
            target,
            "--artifact",
            "output/kernel.hsaco",
            "--source-dir",
            str(self.source),
            "--work-root",
            str(self.work),
            "--timeout",
            "10",
            "--source-date-epoch",
            "12345",
            "--",
            sys.executable,
            str(MOCK_BUILD),
            "--output",
            "{build_dir}/output/kernel.hsaco",
            "--target",
            "{target}",
            "--mode",
            mode,
            "--require-clean-env",
        ]

    def write_result(self, result: reproduce.ReproducibilityResult, name: str) -> Path:
        path = self.root / name
        path.write_bytes(result.canonical_bytes())
        return path

    def test_run_uses_two_clean_directories_and_emits_canonical_result(self) -> None:
        environment = dict(os.environ)
        environment["FE2O3_TEST_POISON"] = "must-not-leak"
        completed = self.run_cli(*self.run_arguments("gfx942"), environment=environment)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        record = self.root / "result.tsv"
        record.write_bytes(completed.stdout)
        result = reproduce.parse_result(record)
        self.assertEqual(result.status, "pass")
        self.assertEqual(result.first_artifact_sha256, result.second_artifact_sha256)
        self.assertEqual(list(self.work.iterdir()), [])

    def test_run_detects_nondeterministic_artifacts(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx1151", "unstable"))
        self.assertEqual(completed.returncode, 1, completed.stderr.decode())
        record = self.root / "mismatch.tsv"
        record.write_bytes(completed.stdout)
        result = reproduce.parse_result(record)
        self.assertEqual(result.status, "fail")
        self.assertEqual(result.reason, "artifact-mismatch")
        self.assertNotEqual(result.first_artifact_sha256, result.second_artifact_sha256)

    def test_run_records_build_failure_missing_artifact_and_unavailable_command(
        self,
    ) -> None:
        cases = (
            (self.run_arguments("gfx950", "fail"), "build-command-failed"),
            (self.run_arguments("gfx950", "missing"), "artifact-unmeasurable"),
            (
                [
                    "run",
                    "--target",
                    "gfx950",
                    "--artifact",
                    "out.hsaco",
                    "--source-dir",
                    str(self.source),
                    "--work-root",
                    str(self.work),
                    "--",
                    "/definitely/unavailable/fe2o3-builder",
                ],
                "build-command-unavailable",
            ),
        )
        for index, (arguments, reason) in enumerate(cases):
            with self.subTest(reason=reason):
                completed = self.run_cli(*arguments)
                self.assertEqual(completed.returncode, 1)
                path = self.root / f"failure-{index}.tsv"
                path.write_bytes(completed.stdout)
                result = reproduce.parse_result(path)
                self.assertEqual(result.reason, reason)

    def test_compare_existing_files_emits_pass_and_fail_records(self) -> None:
        first = self.root / "first.hsaco"
        second = self.root / "second.hsaco"
        first.write_bytes(b"same")
        second.write_bytes(b"same")
        completed = self.run_cli(
            "compare",
            "--target",
            "gfx942:sramecc+:xnack-",
            "--first",
            str(first),
            "--second",
            str(second),
        )
        self.assertEqual(completed.returncode, 0)
        record = self.root / "compare-pass.tsv"
        record.write_bytes(completed.stdout)
        self.assertEqual(reproduce.parse_result(record).status, "pass")

        second.write_bytes(b"different")
        completed = self.run_cli(
            "compare",
            "--target",
            "gfx942:sramecc+:xnack-",
            "--first",
            str(first),
            "--second",
            str(second),
        )
        self.assertEqual(completed.returncode, 1)
        record.write_bytes(completed.stdout)
        self.assertEqual(reproduce.parse_result(record).reason, "artifact-mismatch")

    def test_matrix_requires_exactly_three_passing_base_targets(self) -> None:
        paths: list[Path] = []
        for target, byte in (("gfx1151", "11"), ("gfx942", "42"), ("gfx950", "50")):
            digest = byte * 32
            paths.append(
                self.write_result(
                    reproduce.ReproducibilityResult(
                        target, digest, digest, "pass", "-"
                    ),
                    f"{target}.tsv",
                )
            )
        completed = self.run_cli("matrix", *(str(path) for path in reversed(paths)))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertEqual(
            [line.split(b"\t", 1)[0] for line in completed.stdout.splitlines()],
            [b"gfx1151", b"gfx942", b"gfx950"],
        )

        completed = self.run_cli("matrix", *(str(path) for path in paths[:2]))
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"missing=['gfx950']", completed.stderr)

        completed = self.run_cli("matrix", *(str(path) for path in (*paths, paths[0])))
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"duplicate matrix target", completed.stderr)

    def test_matrix_returns_blocked_when_a_result_did_not_pass(self) -> None:
        records = []
        for target in ("gfx1151", "gfx942"):
            records.append(
                self.write_result(
                    reproduce.ReproducibilityResult(
                        target, "aa" * 32, "aa" * 32, "pass", "-"
                    ),
                    f"{target}.tsv",
                )
            )
        records.append(
            self.write_result(
                reproduce.ReproducibilityResult(
                    "gfx950", "none", "none", "unavailable", "toolchain-unavailable"
                ),
                "gfx950.tsv",
            )
        )
        completed = self.run_cli("matrix", *(str(path) for path in records))
        self.assertEqual(completed.returncode, 1)
        self.assertIn(b"gfx950\tunavailable\ttoolchain-unavailable", completed.stdout)

    def test_parser_rejects_unknown_duplicate_noncanonical_and_truncated_records(
        self,
    ) -> None:
        valid = reproduce.ReproducibilityResult(
            "gfx942", "ab" * 32, "ab" * 32, "pass", "-"
        ).canonical_bytes()
        lines = valid.splitlines(keepends=True)
        cases = {
            "unknown": (lines[:-1] + [b"compiler\tunknown\n", lines[-1]], "unknown"),
            "duplicate": (lines[:-1] + [lines[1], lines[-1]], "duplicate"),
            "noncanonical": ([lines[1], lines[0], *lines[2:]], "noncanonical"),
            "truncated": ([valid[:-1]], "truncated"),
        }
        for name, (parts, diagnostic) in cases.items():
            with self.subTest(name=name):
                path = self.root / f"bad-{name}.tsv"
                path.write_bytes(b"".join(parts))
                with self.assertRaisesRegex(reproduce.EvidenceError, diagnostic):
                    reproduce.parse_result(path)

    def test_invalid_target_and_artifact_traversal_are_rejected(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx999"))
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"target processor", completed.stderr)

        arguments = self.run_arguments("gfx942")
        arguments[arguments.index("output/kernel.hsaco")] = "../kernel.hsaco"
        completed = self.run_cli(*arguments)
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"without traversal", completed.stderr)

    def test_processor_specific_target_features_are_enforced(self) -> None:
        for target in ("gfx1151:xnack+", "gfx1151:sramecc-"):
            with self.subTest(target=target):
                completed = self.run_cli(*self.run_arguments(target))
                self.assertEqual(completed.returncode, 2)
                self.assertIn(b"gfx1151 does not accept", completed.stderr)

        for target in (
            "gfx942:sramecc+:xnack-",
            "gfx950:sramecc-:xnack+",
        ):
            with self.subTest(target=target):
                completed = self.run_cli(*self.run_arguments(target))
                self.assertEqual(completed.returncode, 0, completed.stderr.decode())

    def test_validate_rejects_wrong_target_and_tampering(self) -> None:
        path = self.write_result(
            reproduce.ReproducibilityResult(
                "gfx942", "cd" * 32, "cd" * 32, "pass", "-"
            ),
            "valid.tsv",
        )
        completed = self.run_cli("validate", str(path), "--expect-target", "gfx950")
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"target mismatch", completed.stderr)

        contents = path.read_bytes()
        path.write_bytes(
            contents[:-2] + (b"0" if contents[-2:-1] != b"0" else b"1") + b"\n"
        )
        completed = self.run_cli("validate", str(path), "--expect-target", "gfx942")
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"record_identity mismatch", completed.stderr)


if __name__ == "__main__":
    unittest.main()
