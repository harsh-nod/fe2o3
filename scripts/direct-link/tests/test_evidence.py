#!/usr/bin/env python3

from __future__ import annotations

import dataclasses
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import evidence  # noqa: E402
import reproduce  # noqa: E402
from common import typed_file_identity, typed_identity  # noqa: E402

COMMIT = "12" * 20
TARGET = "gfx942:sramecc+:xnack-"
TOOLCHAIN = typed_identity(reproduce.TOOLCHAIN_DOMAIN, b"llvm-toolchain")
WORKER = typed_identity(reproduce.WORKER_DOMAIN, b"worker")
REQUEST = typed_identity(reproduce.REQUEST_DOMAIN, b"request")
PYTHON = str(Path(sys.executable).resolve(strict=True))


class EvidenceCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.worker = self.root / "worker"
        self.linked = self.root / "linked.hsaco"
        self.final = self.root / "final.hsaco"
        self.worker.write_bytes(b"worker executable")
        self.linked.write_bytes(b"linked hsaco")
        self.final.write_bytes(b"final hsaco")
        self.reproduction = self.root / "reproduction.tsv"
        self.write_reproduction("pass", "-")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_cli(self, *arguments: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(SCRIPT_DIR / "evidence.py"), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def make_reproduction(
        self, status: str, reason: str
    ) -> reproduce.ReproducibilityResult:
        command = reproduce.canonical_json(
            [
                PYTHON,
                "{source_dir}/build.py",
                "--output",
                "{build_dir}/out",
                "--target",
                "{target}",
            ],
            "canonical_argv",
        )
        command_values = reproduce.decode_argv(command)
        first_source = Path("/tmp/fe2o3-first/source")
        first_build = Path("/tmp/fe2o3-first/build")
        second_source = Path("/tmp/fe2o3-second/source")
        second_build = Path("/tmp/fe2o3-second/build")
        environment = reproduce.canonical_json(
            {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
            "environment",
        )
        linked = typed_file_identity(reproduce.LINKED_ARTIFACT_DOMAIN, self.linked)
        finalized = typed_file_identity(reproduce.FINAL_ARTIFACT_DOMAIN, self.final)
        if status != "pass":
            linked_values = ("none", "none")
            final_values = ("none", "none")
        else:
            linked_values = (linked, linked)
            final_values = (finalized, finalized)
        return reproduce.ReproducibilityResult(
            git_commit=COMMIT,
            source_tree_identity=typed_identity(
                reproduce.SOURCE_TREE_DOMAIN, b"source tree"
            ),
            source_snapshot_identity=typed_identity(
                reproduce.SOURCE_SNAPSHOT_DOMAIN, b"checked out source bytes"
            ),
            git_executable_identity=typed_identity(
                reproduce.GIT_EXECUTABLE_DOMAIN, b"git executable"
            ),
            canonical_argv=command,
            canonical_argv_identity=typed_identity(
                reproduce.ARGV_DOMAIN, command.encode("ascii")
            ),
            first_source_dir=str(first_source),
            first_build_dir=str(first_build),
            first_expanded_argv=reproduce.canonical_json(
                reproduce.expanded_command(
                    command_values, first_build, first_source, TARGET
                ),
                "first_expanded_argv",
            ),
            second_source_dir=str(second_source),
            second_build_dir=str(second_build),
            second_expanded_argv=reproduce.canonical_json(
                reproduce.expanded_command(
                    command_values, second_build, second_source, TARGET
                ),
                "second_expanded_argv",
            ),
            build_executable_identity=typed_identity(
                reproduce.EXECUTABLE_DOMAIN, b"build executable"
            ),
            environment=environment,
            environment_identity=typed_identity(
                reproduce.ENVIRONMENT_DOMAIN, environment.encode("ascii")
            ),
            llvm_toolchain_identity=TOOLCHAIN,
            worker_identity=WORKER,
            request_identity=REQUEST,
            target=TARGET,
            first_linked_artifact_identity=linked_values[0],
            second_linked_artifact_identity=linked_values[1],
            first_final_artifact_identity=final_values[0],
            second_final_artifact_identity=final_values[1],
            status=status,
            reason=reason,
        )

    def write_reproduction(self, status: str, reason: str) -> None:
        self.reproduction.write_bytes(
            self.make_reproduction(status, reason).canonical_bytes()
        )

    def collection_arguments(self) -> list[str]:
        return [
            "--git-commit",
            COMMIT,
            "--target",
            TARGET,
            "--worker-executable",
            str(self.worker),
            "--worker-identity",
            WORKER,
            "--llvm-toolchain-identity",
            TOOLCHAIN,
            "--request-identity",
            REQUEST,
            "--linked-artifact",
            str(self.linked),
            "--final-artifact",
            str(self.final),
            "--repro-result",
            str(self.reproduction),
        ]

    def collect_record(self, name: str = "evidence.tsv") -> Path:
        completed = self.run_cli("collect", *self.collection_arguments())
        self.assertEqual(completed.returncode, 1, completed.stderr.decode())
        path = self.root / name
        path.write_bytes(completed.stdout)
        return path

    def test_collection_is_canonical_but_fail_closed(self) -> None:
        path = self.collect_record()
        record = evidence.parse_record(path)
        self.assertEqual(record.scalars["schema_version"], "3")
        self.assertEqual(record.scalars["release_gate"], "blocked")
        self.assertEqual(
            record.suites["clean-build-reproducibility"].status, "unavailable"
        )
        self.assertEqual(
            record.suites["clean-build-reproducibility"].reason,
            "unauthenticated-reproducibility",
        )
        for name in (
            "compile",
            "direct-llvm-link",
            "hardware-execution",
            "static-checks",
        ):
            self.assertEqual(record.suites[name].status, "unavailable")
            self.assertEqual(record.suites[name].provenance_identity, "none")

    def test_inspect_accepts_blocked_record_validate_does_not(self) -> None:
        path = self.collect_record()
        inspect = self.run_cli("inspect", str(path))
        self.assertEqual(inspect.returncode, 0, inspect.stderr.decode())
        validate = self.run_cli("validate", str(path), *self.collection_arguments())
        self.assertEqual(validate.returncode, 1, validate.stderr.decode())
        self.assertIn(b"gate=blocked", validate.stdout)

    def test_failed_reproduction_makes_release_gate_fail(self) -> None:
        self.write_reproduction("fail", "artifact-mismatch")
        path = self.collect_record("failed.tsv")
        record = evidence.parse_record(path)
        self.assertEqual(record.scalars["release_gate"], "fail")
        self.assertEqual(self.run_cli("inspect", str(path)).returncode, 0)
        self.assertEqual(
            self.run_cli(
                "validate", str(path), *self.collection_arguments()
            ).returncode,
            1,
        )

    def test_arbitrary_cli_suite_pass_is_not_an_input_surface(self) -> None:
        completed = self.run_cli(
            "collect",
            *self.collection_arguments(),
            "--suite",
            "hardware-execution=pass",
        )
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"unrecognized arguments", completed.stderr)

    def test_forged_suite_passes_are_rejected(self) -> None:
        record = evidence.parse_record(self.collect_record())
        for suite_name in (
            "clean-build-reproducibility",
            "compile",
            "hardware-execution",
        ):
            with self.subTest(suite=suite_name):
                suites = dict(record.suites)
                original = suites[suite_name]
                suites[suite_name] = dataclasses.replace(
                    original,
                    status="pass",
                    reason="-",
                    provenance_identity=typed_identity(
                        "fe2o3-forged-provenance-v1", b"did not run"
                    ),
                )
                scalars = dict(record.scalars)
                scalars["release_gate"] = "blocked"
                forged = evidence.EvidenceRecord(scalars, suites)
                path = self.root / f"forged-{suite_name}.tsv"
                path.write_bytes(forged.canonical_bytes())
                with self.assertRaisesRegex(
                    evidence.EvidenceError, "cannot pass until"
                ):
                    evidence.parse_record(path)

    def test_reproduction_provenance_and_artifact_substitution_fail(self) -> None:
        path = self.collect_record()
        record = evidence.parse_record(path)
        suites = dict(record.suites)
        suites["clean-build-reproducibility"] = dataclasses.replace(
            suites["clean-build-reproducibility"],
            provenance_identity=typed_identity(
                reproduce.RECORD_DOMAIN, b"different reproduction"
            ),
        )
        forged = evidence.EvidenceRecord(dict(record.scalars), suites)
        forged_path = self.root / "forged-reproduction.tsv"
        forged_path.write_bytes(forged.canonical_bytes())
        with self.assertRaisesRegex(evidence.EvidenceError, "does not bind"):
            evidence.parse_record(forged_path)

        self.final.write_bytes(b"substituted final artifact")
        completed = self.run_cli("validate", str(path), *self.collection_arguments())
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"does not match evidence", completed.stderr)

    def test_request_and_toolchain_substitution_fail(self) -> None:
        path = self.collect_record()
        for option, value in (
            (
                "--request-identity",
                typed_identity(reproduce.REQUEST_DOMAIN, b"other request"),
            ),
            (
                "--llvm-toolchain-identity",
                typed_identity(reproduce.TOOLCHAIN_DOMAIN, b"other toolchain"),
            ),
        ):
            with self.subTest(option=option):
                arguments = self.collection_arguments()
                arguments[arguments.index(option) + 1] = value
                completed = self.run_cli("validate", str(path), *arguments)
                self.assertEqual(completed.returncode, 2)
                self.assertIn(b"does not match evidence", completed.stderr)

    def test_record_identity_uses_domain_separation_and_detects_tampering(self) -> None:
        path = self.collect_record()
        record = evidence.parse_record(path)
        self.assertNotEqual(
            record.identity().rsplit("-", 1)[1],
            typed_identity("fe2o3-other-record-v1", record.preimage()).rsplit("-", 1)[
                1
            ],
        )
        data = path.read_bytes()
        path.write_bytes(data[:-2] + (b"0" if data[-2:-1] != b"0" else b"1") + b"\n")
        with self.assertRaisesRegex(evidence.EvidenceError, "does not authenticate"):
            evidence.parse_record(path)

    @unittest.skipUnless(hasattr(os, "symlink"), "symlinks unavailable")
    def test_measurement_does_not_follow_symlinks(self) -> None:
        path = self.collect_record()
        link = self.root / "evidence-link.tsv"
        link.symlink_to(path)
        with self.assertRaisesRegex(evidence.EvidenceError, "cannot open regular file"):
            evidence.parse_record(link)


if __name__ == "__main__":
    unittest.main()
