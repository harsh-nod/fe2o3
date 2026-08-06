#!/usr/bin/env python3

from __future__ import annotations

import dataclasses
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parents[1]
MOCK_BUILD = Path(__file__).resolve().with_name("mock_build.py")
sys.path.insert(0, str(SCRIPT_DIR))

import reproduce  # noqa: E402
from common import typed_identity  # noqa: E402

TOOLCHAIN = typed_identity(reproduce.TOOLCHAIN_DOMAIN, b"llvm-22-test-toolchain")
WORKER = typed_identity(reproduce.WORKER_DOMAIN, b"worker-test-build")
REQUEST = typed_identity(reproduce.REQUEST_DOMAIN, b"canonical-link-request")
PYTHON = str(Path(sys.executable).resolve(strict=True))


class ReproducibilityCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.work = self.root / "work"
        self.source.mkdir()
        self.work.mkdir()
        shutil.copyfile(MOCK_BUILD, self.source / "mock_build.py")
        self.git("init", "--quiet")
        self.git("config", "user.name", "G8 Test")
        self.git("config", "user.email", "g8@example.invalid")
        self.git("add", "mock_build.py")
        self.git("commit", "--quiet", "-m", "fixture")
        self.commit = self.git("rev-parse", "HEAD").stdout.decode("ascii").strip()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *arguments: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            ["git", "-C", str(self.source), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )

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

    def run_arguments(
        self, target: str, mode: str = "stable", marker: Path | None = None
    ) -> list[str]:
        arguments = [
            "run",
            "--commit",
            self.commit,
            "--target",
            target,
            "--linked-artifact",
            "output/linked.hsaco",
            "--final-artifact",
            "output/final.hsaco",
            "--source-dir",
            str(self.source),
            "--work-root",
            str(self.work),
            "--timeout",
            "1",
            "--llvm-toolchain-identity",
            TOOLCHAIN,
            "--worker-identity",
            WORKER,
            "--request-identity",
            REQUEST,
            "--",
            PYTHON,
            "{source_dir}/mock_build.py",
            "--linked",
            "{build_dir}/output/linked.hsaco",
            "--final",
            "{build_dir}/output/final.hsaco",
            "--target",
            "{target}",
            "--mode",
            mode,
            "--require-clean-env",
        ]
        if marker is not None:
            arguments.extend(("--marker", str(marker)))
        return arguments

    def parse_stdout(
        self, completed: subprocess.CompletedProcess[bytes], name: str = "result.tsv"
    ) -> reproduce.ReproducibilityResult:
        path = self.root / name
        path.write_bytes(completed.stdout)
        return reproduce.parse_result(path)

    def write_result(self, result: reproduce.ReproducibilityResult, name: str) -> Path:
        path = self.root / name
        path.write_bytes(result.canonical_bytes())
        return path

    def validate_arguments(
        self, path: Path, result: reproduce.ReproducibilityResult
    ) -> list[str]:
        return [
            "validate",
            str(path),
            "--expect-commit",
            result.git_commit,
            "--expect-source-tree-identity",
            result.source_tree_identity,
            "--expect-argv-identity",
            result.canonical_argv_identity,
            "--build-executable",
            PYTHON,
            "--expect-environment-identity",
            result.environment_identity,
            "--expect-llvm-toolchain-identity",
            result.llvm_toolchain_identity,
            "--expect-worker-identity",
            result.worker_identity,
            "--expect-request-identity",
            result.request_identity,
            "--expect-target",
            result.target,
        ]

    def test_run_uses_two_clean_detached_snapshots_and_emits_v2(self) -> None:
        environment = dict(os.environ)
        environment["FE2O3_TEST_POISON"] = "must-not-leak"
        completed = self.run_cli(*self.run_arguments("gfx942"), environment=environment)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        result = self.parse_stdout(completed)
        self.assertEqual(result.status, "pass")
        self.assertEqual(
            result.first_linked_artifact_identity,
            result.second_linked_artifact_identity,
        )
        self.assertEqual(
            result.first_final_artifact_identity,
            result.second_final_artifact_identity,
        )
        self.assertEqual(list(self.work.iterdir()), [])
        self.assertEqual(self.git("status", "--porcelain").stdout, b"")

    def test_domain_is_part_of_identity_preimage(self) -> None:
        payload = b"same payload"
        first = typed_identity("fe2o3-domain-one-v1", payload)
        second = typed_identity("fe2o3-domain-two-v1", payload)
        self.assertNotEqual(first.rsplit("-", 1)[1], second.rsplit("-", 1)[1])

    def test_detects_nondeterministic_artifacts(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx1151", "unstable"))
        self.assertEqual(completed.returncode, 1, completed.stderr.decode())
        result = self.parse_stdout(completed)
        self.assertEqual(result.status, "fail")
        self.assertEqual(result.reason, "artifact-mismatch")

    def test_large_artifacts_are_not_limited_by_log_bound(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx950", "large"))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertEqual(self.parse_stdout(completed).status, "pass")

    def test_timeout_kills_build(self) -> None:
        started = time.monotonic()
        completed = self.run_cli(*self.run_arguments("gfx942", "timeout"))
        self.assertLess(time.monotonic() - started, 4)
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(self.parse_stdout(completed).reason, "build-timeout")

    def test_post_exit_descendant_mutation_is_cleaned(self) -> None:
        marker = self.root / "descendant-marker"
        completed = self.run_cli(*self.run_arguments("gfx942", "descendant", marker))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        time.sleep(0.8)
        self.assertFalse(marker.exists())

    def test_parent_owned_log_capture_is_bounded(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942", "noisy"))
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(self.parse_stdout(completed).reason, "build-log-limit")

    def test_source_mutation_is_rejected_and_original_is_unchanged(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942", "mutate-source"))
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(self.parse_stdout(completed).reason, "source-snapshot-mutated")
        self.assertFalse((self.source / "source-mutation.txt").exists())

    def test_fail_missing_and_unavailable_commands_are_explicit(self) -> None:
        cases = (
            (self.run_arguments("gfx950", "fail"), "build-command-failed"),
            (self.run_arguments("gfx950", "missing"), "artifact-unmeasurable"),
        )
        for index, (arguments, reason) in enumerate(cases):
            with self.subTest(reason=reason):
                completed = self.run_cli(*arguments)
                self.assertEqual(completed.returncode, 1)
                self.assertEqual(
                    self.parse_stdout(completed, f"failure-{index}.tsv").reason,
                    reason,
                )

        arguments = self.run_arguments("gfx950")
        separator = arguments.index("--")
        arguments[separator + 1] = "/definitely/unavailable/fe2o3-builder"
        completed = self.run_cli(*arguments)
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"cannot open regular file", completed.stderr)

    def test_inspect_accepts_failure_but_validate_returns_nonzero(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942", "unstable"))
        result = self.parse_stdout(completed)
        path = self.write_result(result, "failure.tsv")
        self.assertEqual(self.run_cli("inspect", str(path)).returncode, 0)
        self.assertEqual(
            self.run_cli(*self.validate_arguments(path, result)).returncode, 1
        )

    def test_validate_rejects_command_source_and_toolchain_substitution(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942"))
        result = self.parse_stdout(completed)
        path = self.write_result(result, "valid.tsv")
        base = self.validate_arguments(path, result)
        substitutions = {
            "--expect-argv-identity": typed_identity(
                reproduce.ARGV_DOMAIN, b"substituted command"
            ),
            "--expect-source-tree-identity": typed_identity(
                reproduce.SOURCE_TREE_DOMAIN, b"substituted source"
            ),
            "--expect-llvm-toolchain-identity": typed_identity(
                reproduce.TOOLCHAIN_DOMAIN, b"substituted toolchain"
            ),
        }
        for option, value in substitutions.items():
            with self.subTest(option=option):
                arguments = list(base)
                arguments[arguments.index(option) + 1] = value
                rejected = self.run_cli(*arguments)
                self.assertEqual(rejected.returncode, 2)
                self.assertIn(b"mismatch", rejected.stderr)

    def test_record_rejects_forged_argv_identity_and_tampering(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942"))
        result = self.parse_stdout(completed)
        forged = dataclasses.replace(
            result,
            canonical_argv_identity=typed_identity(
                reproduce.ARGV_DOMAIN, b"unbound command"
            ),
        )
        path = self.write_result(forged, "forged.tsv")
        with self.assertRaisesRegex(reproduce.EvidenceError, "argv_identity mismatch"):
            reproduce.parse_result(path)

        valid = self.write_result(result, "tampered.tsv")
        data = valid.read_bytes()
        valid.write_bytes(data[:-2] + (b"0" if data[-2:-1] != b"0" else b"1") + b"\n")
        with self.assertRaisesRegex(
            reproduce.EvidenceError, "record_identity mismatch"
        ):
            reproduce.parse_result(valid)

    def test_invalid_target_features_and_artifact_traversal_are_rejected(self) -> None:
        for target in ("gfx999", "gfx1151:xnack+", "gfx942:xnack+:sramecc-"):
            with self.subTest(target=target):
                completed = self.run_cli(*self.run_arguments(target))
                self.assertEqual(completed.returncode, 2)
        arguments = self.run_arguments("gfx942")
        arguments[arguments.index("output/final.hsaco")] = "../final.hsaco"
        completed = self.run_cli(*arguments)
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"without traversal", completed.stderr)


if __name__ == "__main__":
    unittest.main()
