#!/usr/bin/env python3

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import common  # noqa: E402
import evidence  # noqa: E402
import reproduce  # noqa: E402

COMMIT = "01" * 20
REQUEST = "02" * 32
WORKER_BUILD = "fe2o3-worker-v1-sha256-" + "03" * 32
LLVM_BUILD = "rocm-llvm-dev=22.0.0.26084.70204-93~24.04"
HARDWARE_ID = "fe2o3-hardware-v1-sha256-" + "04" * 32


class EvidenceCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.worker = self.root / "worker"
        self.artifact = self.root / "linked.hsaco"
        self.worker.write_bytes(b"measured worker executable\n")
        self.artifact.write_bytes(b"deterministic linked artifact\n")
        self.worker.chmod(0o700)
        artifact_digest = evidence.sha256_file(self.artifact)
        result = reproduce.ReproducibilityResult(
            "gfx942", artifact_digest, artifact_digest, "pass", "-"
        )
        self.reproduction = self.root / "repro.tsv"
        self.reproduction.write_bytes(result.canonical_bytes())

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_cli(
        self, *arguments: str, check: bool = False
    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(SCRIPT_DIR / "evidence.py"), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=check,
        )

    def collect_arguments(self, hardware: str = "pass") -> list[str]:
        hardware_argument = (
            ["--hardware-execution-identity", HARDWARE_ID] if hardware == "pass" else []
        )
        suites = [
            "clean-build-reproducibility=pass",
            "compile=pass",
            "direct-llvm-link=pass",
            (
                "hardware-execution=pass"
                if hardware == "pass"
                else "hardware-execution=unavailable:no-compatible-gpu"
            ),
            "static-checks=pass",
        ]
        arguments = [
            "collect",
            "--git-commit",
            COMMIT,
            "--target",
            "gfx942",
            "--worker-executable",
            str(self.worker),
            "--worker-build-id",
            WORKER_BUILD,
            "--llvm-build-identity",
            LLVM_BUILD,
            "--request-identity",
            REQUEST,
            "--artifact",
            str(self.artifact),
            *hardware_argument,
        ]
        for suite in suites:
            arguments.extend(("--suite", suite))
        return arguments

    def collect_record(self, hardware: str = "pass") -> Path:
        completed = self.run_cli(*self.collect_arguments(hardware), check=True)
        path = self.root / f"evidence-{hardware}.tsv"
        path.write_bytes(completed.stdout)
        return path

    def validate_arguments(self, record: Path) -> list[str]:
        return [
            "validate",
            str(record),
            "--expect-commit",
            COMMIT,
            "--expect-target",
            "gfx942",
            "--worker-executable",
            str(self.worker),
            "--expect-worker-build-id",
            WORKER_BUILD,
            "--expect-llvm-build-identity",
            LLVM_BUILD,
            "--expect-request-identity",
            REQUEST,
            "--artifact",
            str(self.artifact),
            "--repro-result",
            str(self.reproduction),
        ]

    def test_collect_is_deterministic_and_validate_pins_every_identity(self) -> None:
        first = self.run_cli(*self.collect_arguments(), check=True).stdout
        second = self.run_cli(*self.collect_arguments(), check=True).stdout
        self.assertEqual(first, second)
        record = self.root / "evidence.tsv"
        record.write_bytes(first)
        parsed = evidence.parse_record(record)
        self.assertEqual(parsed.scalars["release_gate"], "pass")
        completed = self.run_cli(*self.validate_arguments(record))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())

    def test_explicit_hardware_unavailability_blocks_release(self) -> None:
        record = self.collect_record("unavailable")
        parsed = evidence.parse_record(record)
        self.assertEqual(parsed.scalars["release_gate"], "blocked")
        self.assertEqual(parsed.scalars["hardware_execution_identity"], "none")

    def test_unavailable_or_skipped_suite_requires_a_reason(self) -> None:
        arguments = self.collect_arguments("unavailable")
        index = arguments.index("hardware-execution=unavailable:no-compatible-gpu")
        arguments[index] = "hardware-execution=unavailable"
        completed = self.run_cli(*arguments)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn(b"reason must be", completed.stderr)

    def test_wrong_authenticated_pins_are_rejected(self) -> None:
        record = self.collect_record()
        cases = {
            "commit": ("--expect-commit", "ab" * 20, b"git_commit mismatch"),
            "target": ("--expect-target", "gfx950", b"target mismatch"),
            "worker build": (
                "--expect-worker-build-id",
                "fe2o3-worker-v1-sha256-" + "aa" * 32,
                b"worker_build_id mismatch",
            ),
            "LLVM build": (
                "--expect-llvm-build-identity",
                "rocm-llvm-dev=wrong",
                b"llvm_build_identity mismatch",
            ),
            "request": (
                "--expect-request-identity",
                "bb" * 32,
                b"request_identity mismatch",
            ),
        }
        base = self.validate_arguments(record)
        for name, (option, replacement, expected) in cases.items():
            with self.subTest(name=name):
                arguments = list(base)
                arguments[arguments.index(option) + 1] = replacement
                completed = self.run_cli(*arguments)
                self.assertEqual(completed.returncode, 2)
                self.assertIn(expected, completed.stderr)

    def test_changed_worker_and_artifact_bytes_are_rejected(self) -> None:
        record = self.collect_record()
        self.worker.write_bytes(b"replaced worker\n")
        completed = self.run_cli(*self.validate_arguments(record))
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"worker_executable_sha256 mismatch", completed.stderr)

        self.worker.write_bytes(b"measured worker executable\n")
        self.artifact.write_bytes(b"replaced artifact\n")
        completed = self.run_cli(*self.validate_arguments(record))
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"artifact_identity mismatch", completed.stderr)

    def test_hash_rejects_same_inode_mutation_and_opens_the_path_once(self) -> None:
        measured = self.root / "measured.bin"
        measured.write_bytes(b"a" * 4096)
        original_read = common.os.read
        mutated = False

        def mutate_after_read(descriptor: int, size: int) -> bytes:
            nonlocal mutated
            data = original_read(descriptor, size)
            if data and not mutated:
                mutated = True
                with measured.open("r+b") as output:
                    output.seek(0, os.SEEK_END)
                    output.write(b"b")
                    output.flush()
                    os.fsync(output.fileno())
            return data

        with mock.patch.object(common.os, "read", side_effect=mutate_after_read):
            with self.assertRaisesRegex(
                common.EvidenceError, "changed while being measured"
            ):
                common.sha256_file(measured)

        measured.write_bytes(b"stable")
        original_open = common.os.open
        open_calls = 0

        def count_open(*args: object, **kwargs: object) -> int:
            nonlocal open_calls
            open_calls += 1
            return original_open(*args, **kwargs)

        with mock.patch.object(common.os, "open", side_effect=count_open):
            common.sha256_file(measured)
        self.assertEqual(open_calls, 1)

    def test_reproducibility_record_is_bound_to_target_outcome_and_artifact(
        self,
    ) -> None:
        record = self.collect_record()
        digest = evidence.sha256_file(self.artifact)
        cases = (
            reproduce.ReproducibilityResult("gfx950", digest, digest, "pass", "-"),
            reproduce.ReproducibilityResult(
                "gfx942", "aa" * 32, "aa" * 32, "pass", "-"
            ),
            reproduce.ReproducibilityResult(
                "gfx942", "none", "none", "unavailable", "toolchain-unavailable"
            ),
        )
        expected = (
            b"reproducibility target",
            b"does not bind the recorded artifact",
            b"outcome does not match",
        )
        for index, result in enumerate(cases):
            with self.subTest(index=index):
                self.reproduction.write_bytes(result.canonical_bytes())
                completed = self.run_cli(*self.validate_arguments(record))
                self.assertEqual(completed.returncode, 2)
                self.assertIn(expected[index], completed.stderr)

    def test_parser_rejects_duplicate_unknown_noncanonical_and_truncated_data(
        self,
    ) -> None:
        record = self.collect_record()
        original = record.read_bytes()
        lines = original.splitlines(keepends=True)
        mutations = {
            "duplicate": lines[:-1] + [lines[1], lines[-1]],
            "unknown": lines[:-1] + [b"environment\tpoisoned\n", lines[-1]],
            "noncanonical": [lines[1], lines[0], *lines[2:]],
        }
        expected = {
            "duplicate": "duplicate scalar field",
            "unknown": "unknown field",
            "noncanonical": "noncanonical field order",
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                path = self.root / f"{name}.tsv"
                path.write_bytes(b"".join(mutation))
                with self.assertRaisesRegex(evidence.EvidenceError, expected[name]):
                    evidence.parse_record(path)

        truncated = self.root / "truncated.tsv"
        truncated.write_bytes(original[:-1])
        with self.assertRaisesRegex(evidence.EvidenceError, "truncated"):
            evidence.parse_record(truncated)

    def test_parser_rejects_overlong_and_non_ascii_values(self) -> None:
        record = self.collect_record()
        parsed = evidence.parse_record(record)
        scalars = dict(parsed.scalars)
        scalars["llvm_build_identity"] = "x" * 193
        overlong = evidence.EvidenceRecord(scalars, parsed.suites)
        path = self.root / "overlong.tsv"
        path.write_bytes(overlong.canonical_bytes())
        with self.assertRaisesRegex(evidence.EvidenceError, "exceeds 192"):
            evidence.parse_record(path)

        path.write_bytes(
            record.read_bytes().replace(LLVM_BUILD.encode(), b"LLVM-\xc3\xa9")
        )
        with self.assertRaisesRegex(evidence.EvidenceError, "ASCII"):
            evidence.parse_record(path)

    def test_compile_only_cannot_claim_hardware_execution(self) -> None:
        record = self.collect_record("unavailable")
        parsed = evidence.parse_record(record)

        suites = dict(parsed.suites)
        suites["hardware-execution"] = evidence.SuiteOutcome(
            "hardware-execution", "hardware", "pass", "-"
        )
        forged = evidence.EvidenceRecord(dict(parsed.scalars), suites)
        path = self.root / "forged-hardware.tsv"
        path.write_bytes(forged.canonical_bytes())
        with self.assertRaisesRegex(evidence.EvidenceError, "hardware pass requires"):
            evidence.parse_record(path)

        suites["hardware-execution"] = evidence.SuiteOutcome(
            "hardware-execution", "compile", "pass", "-"
        )
        scalars = dict(parsed.scalars)
        scalars["hardware_execution_identity"] = HARDWARE_ID
        path.write_bytes(evidence.EvidenceRecord(scalars, suites).canonical_bytes())
        with self.assertRaisesRegex(evidence.EvidenceError, "wrong evidence class"):
            evidence.parse_record(path)

    def test_release_gate_cannot_overclaim_a_skipped_suite(self) -> None:
        record = self.collect_record("unavailable")
        parsed = evidence.parse_record(record)
        scalars = dict(parsed.scalars)
        scalars["release_gate"] = "pass"
        forged = evidence.EvidenceRecord(scalars, parsed.suites)
        path = self.root / "overclaim.tsv"
        path.write_bytes(forged.canonical_bytes())
        with self.assertRaisesRegex(evidence.EvidenceError, "overclaims outcomes"):
            evidence.parse_record(path)

    @unittest.skipUnless(hasattr(os, "symlink"), "symlinks unavailable")
    def test_validator_does_not_follow_evidence_or_binary_symlinks(self) -> None:
        record = self.collect_record()
        record_link = self.root / "record-link.tsv"
        record_link.symlink_to(record)
        with self.assertRaisesRegex(evidence.EvidenceError, "cannot open regular file"):
            evidence.parse_record(record_link)

        worker_link = self.root / "worker-link"
        worker_link.symlink_to(self.worker)
        arguments = self.validate_arguments(record)
        arguments[arguments.index("--worker-executable") + 1] = str(worker_link)
        completed = self.run_cli(*arguments)
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"cannot open regular file", completed.stderr)


if __name__ == "__main__":
    unittest.main()
